use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::psx::cop0;
use crate::psx::cpu::RegisterIndex;
use crate::psx::Psx;

use crate::debugger::Debugger;

use self::reply::Reply;

pub(super) mod reply;

pub type GdbResult = Result<(), ()>;

pub struct GdbRemote {
    remote: TcpStream,
}

impl GdbRemote {
    pub fn new(listener: &TcpListener) -> GdbRemote {
        info!("Debugger waiting for gdb connection...");

        let remote = match listener.accept() {
            Ok((stream, sockaddr)) => {
                info!("Connection from {}", sockaddr);
                stream
            }
            Err(e) => panic!("Accept failed: {}", e),
        };

        GdbRemote { remote }
    }

    // Serve a single remote request
    pub fn serve(&mut self, debugger: &mut Debugger, psx: &mut Psx) -> GdbResult {
        match self.next_packet() {
            PacketResult::Ok(packet) => {
                self.ack()?;
                self.handle_packet(debugger, psx, &packet)
            }
            PacketResult::BadChecksum(_) => {
                // Request retransmission
                self.nack()
            }
            PacketResult::EndOfStream => {
                // Session over
                Err(())
            }
        }
    }

    /// Attempt to return a single GDB packet.
    fn next_packet(&mut self) -> PacketResult {
        // Parser state machine
        enum State {
            WaitForStart,
            InPacket,
            WaitForCheckSum,
            WaitForCheckSum2(u8),
        }

        let mut state = State::WaitForStart;

        let mut packet = Vec::new();
        let mut csum = 0u8;

        for r in (&self.remote).bytes() {
            let byte = match r {
                Ok(b) => b,
                Err(e) => {
                    warn!("GDB remote error: {}", e);
                    return PacketResult::EndOfStream;
                }
            };

            match state {
                State::WaitForStart => {
                    if byte == b'$' {
                        // Start of packet
                        state = State::InPacket;
                    }
                }
                State::InPacket => {
                    if byte == b'#' {
                        // End of packet
                        state = State::WaitForCheckSum;
                    } else {
                        // Append byte to the packet
                        packet.push(byte);
                        // Update checksum
                        csum = csum.wrapping_add(byte);
                    }
                }
                State::WaitForCheckSum => match ascii_hex(byte) {
                    Some(b) => {
                        state = State::WaitForCheckSum2(b);
                    }
                    None => {
                        warn!("Got invalid GDB checksum char {}", byte);
                        return PacketResult::BadChecksum(packet);
                    }
                },
                State::WaitForCheckSum2(c1) => {
                    match ascii_hex(byte) {
                        Some(c2) => {
                            let expected = (c1 << 4) | c2;

                            if expected != csum {
                                warn!("Got invalid GDB checksum: {:x} {:x}", expected, csum);
                                return PacketResult::BadChecksum(packet);
                            }

                            // Checksum is good, we're done!
                            return PacketResult::Ok(packet);
                        }
                        None => {
                            warn!("Got invalid GDB checksum char {}", byte);
                            return PacketResult::BadChecksum(packet);
                        }
                    }
                }
            }
        }

        warn!("GDB remote end of stream");
        PacketResult::EndOfStream
    }

    /// Acknowledge packet reception
    fn ack(&mut self) -> GdbResult {
        if let Err(e) = self.remote.write(b"+") {
            warn!("Couldn't send ACK to GDB remote: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }

    /// Request packet retransmission
    fn nack(&mut self) -> GdbResult {
        if let Err(e) = self.remote.write(b"-") {
            warn!("Couldn't send NACK to GDB remote: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }

    fn handle_packet(
        &mut self,
        debugger: &mut Debugger,
        psx: &mut Psx,
        packet: &[u8],
    ) -> GdbResult {
        let command = packet[0];
        let args = &packet[1..];

        let res = match command {
            b'?' => self.send_status(),
            b'm' => self.read_memory(psx, args),
            b'M' => self.write_memory(psx, args),
            b'g' => self.read_registers(psx),
            b'G' => self.write_registers(psx, args),
            b'P' => self.write_register(psx, args),
            b'c' => self.resume(debugger, psx, args),
            b's' => self.step(debugger, psx, args),
            b'Z' => self.add_breakpoint(debugger, args),
            b'z' => self.del_breakpoint(debugger, args),
            b'q' => self.handle_query(debugger, args),
            b'Q' => self.handle_set(args),
            b'k' => self.kill(),
            b'D' => self.detach(debugger),
            b'H' => self.set_thread(args),
            b'T' => self.thread_alive(args),
            b'v' => self.handle_v_packet(debugger, psx, args),
            // Send empty response for unsupported packets
            _ => self.send_empty_reply(),
        };

        // Check for errors
        res?;

        Ok(())
    }

    fn send_reply(&mut self, reply: Reply) -> GdbResult {
        match self.remote.write(&reply.into_packet()) {
            // XXX Should we check the number of bytes written? What
            // do we do if it's less than we expected, retransmit?
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("Couldn't send data to GDB remote: {}", e);
                Err(())
            }
        }
    }

    fn send_empty_reply(&mut self) -> GdbResult {
        self.send_reply(Reply::new())
    }

    fn send_string(&mut self, string: &[u8]) -> GdbResult {
        let mut reply = Reply::new();

        reply.push(string);

        self.send_reply(reply)
    }

    fn send_error(&mut self) -> GdbResult {
        // GDB remote doesn't specify what the error codes should
        // be. Should be bother coming up with our own convention?
        self.send_string(b"E00")
    }

    pub fn send_status(&mut self) -> GdbResult {
        // Maybe we should return different values depending on the
        // cause of the "break"?
        self.send_string(b"S00")
    }

    pub fn send_ok(&mut self) -> GdbResult {
        self.send_string(b"OK")
    }

    fn read_registers(&mut self, psx: &mut Psx) -> GdbResult {
        let mut reply = Reply::new();

        // Send general purpose registers
        for &r in psx.cpu.regs() {
            reply.push_u32(r);
        }

        // Send control registers
        for &r in &[
            psx.cop0.sr(),
            psx.cpu.lo(),
            psx.cpu.hi(),
            psx.cop0.bad(),
            cop0::cause(psx),
            psx.cpu.current_pc(),
        ] {
            reply.push_u32(r);
        }

        // GDB expects 73 registers for the MIPS architecture: the 38 above plus all the floating
        // point registers. Since the playstation doesn't support those we just return `x`s to
        // notify GDB that those registers are unavailable.
        //
        // The doc says that it's normally used for core dumps however (when the value of a
        // register can't be found in a trace) so I'm not sure it's the right thing to do. If it
        // causes problems we might just return 0 (or some sane default value) instead.
        for _ in 38..73 {
            reply.push(b"xxxxxxxx");
        }

        self.send_reply(reply)
    }

    fn examine_word(&mut self, psx: &mut Psx, addr: u32) -> u32 {
        // When dumping 32bit values we reserve an unused memory range to dump values that can't be
        // accessed otherwise with GDB, such as coprocessor registers
        if (0xbad0_0000..=0xbad0_ffff).contains(&addr) {
            let off = (addr - 0xbad0_0000) / 4;

            match off {
                0..=31 => cop0::mfc0(psx, RegisterIndex(off as u8)),
                _ => 0x0bad_0bad,
            }
        } else {
            psx.examine(addr)
        }
    }

    /// Read a region of memory. The packet format should be
    /// `ADDR,LEN`, both in hexadecimal
    fn read_memory(&mut self, psx: &mut Psx, args: &[u8]) -> GdbResult {
        let mut reply = Reply::new();

        let (addr, len) = parse_addr_len(args)?;

        if len == 0 {
            // Should we reply with an empty string here? Probably
            // doesn't matter
            return self.send_error();
        }

        // We can now fetch the data. First we handle the case where
        // addr is not aligned using an ad-hoc heuristic. A better way
        // to do this might be to figure out which peripheral we're
        // accessing and select the most meaningful access width.
        let align = addr % 4;

        let sent = match align {
            1 | 3 => {
                // If we fall on the first or third byte of a word
                // we use byte accesses until we reach the next
                // word or the end of the requested length
                let count = ::std::cmp::min(len, 4 - align);

                for i in 0..count {
                    let b: u8 = psx.examine(addr.wrapping_add(i));
                    reply.push_u8(b as u8);
                }
                count
            }
            2 => {
                if len == 1 {
                    // Only one byte to read
                    reply.push_u8(psx.examine(addr));
                    1
                } else {
                    reply.push_u16(psx.examine(addr));
                    2
                }
            }
            _ => 0,
        };

        let addr = addr.wrapping_add(sent);
        let len = len - sent;

        // We can now deal with the word-aligned portion of the
        // transfer (if any). It's possible that addr is not word
        // aligned here if we entered the case "align == 2, len == 1"
        // above but it doesn't matter because in this case "nwords"
        // will be 0 so nothing will be fetched.
        let nwords = len / 4;

        for i in 0..nwords {
            let w = self.examine_word(psx, addr + i * 4);
            reply.push_u32(w);
        }

        // See if we have anything remaining
        let addr = addr.wrapping_add(nwords * 4);
        let rem = len - nwords * 4;

        match rem {
            1 | 3 => {
                for i in 0..rem {
                    let b = psx.examine(addr.wrapping_add(i));
                    reply.push_u8(b);
                }
            }
            2 => {
                reply.push_u16(psx.examine(addr));
            }
            _ => (),
        }

        self.send_reply(reply)
    }

    /// Continue execution
    fn resume(&mut self, debugger: &mut Debugger, psx: &mut Psx, args: &[u8]) -> GdbResult {
        if !args.is_empty() {
            // If an address is provided we restart from there
            let addr = parse_hex(args)?;

            psx.cpu.force_pc(addr);
        }

        // Tell the debugger we want to resume execution.
        debugger.resume();

        Ok(())
    }

    // Step works exactly like continue except that we're only
    // supposed to execute a single instruction.
    fn step(&mut self, debugger: &mut Debugger, psx: &mut Psx, args: &[u8]) -> GdbResult {
        debugger.set_step();

        self.resume(debugger, psx, args)
    }

    // Add a breakpoint or watchpoint
    fn add_breakpoint(&mut self, debugger: &mut Debugger, args: &[u8]) -> GdbResult {
        // Check if the request contains a command list
        if args.iter().any(|&b| b == b';') {
            // Not sure if I should signal an error or send an empty
            // reply here to signal that command lists are not
            // supported. I think GDB will think that we don't support
            // this breakpoint type *at all* if we return an empty
            // reply. I don't know how it handles errors however.
            return self.send_error();
        };

        let (btype, addr, kind) = parse_breakpoint(args)?;

        // Only kind "4" makes sense for us: 32bits standard MIPS mode
        // breakpoint. The MIPS-specific kinds are defined here:
        // https://sourceware.org/gdb/onlinedocs/gdb/MIPS-Breakpoint-Kinds.html
        if kind != b'4' {
            // Same question as above, should I signal an error?
            return self.send_error();
        }

        match btype {
            b'0' => debugger.add_breakpoint(addr),
            b'2' => debugger.add_write_watchpoint(addr),
            b'3' => debugger.add_read_watchpoint(addr),
            // Unsupported breakpoint type
            _ => return self.send_empty_reply(),
        }

        self.send_ok()
    }

    // Delete a breakpoint or watchpoint
    fn del_breakpoint(&mut self, debugger: &mut Debugger, args: &[u8]) -> GdbResult {
        let (btype, addr, kind) = parse_breakpoint(args)?;

        // Only 32bits standard MIPS mode breakpoint supported
        if kind != b'4' {
            return self.send_error();
        }

        match btype {
            b'0' => debugger.del_breakpoint(addr),
            b'2' => debugger.del_write_watchpoint(addr),
            b'3' => debugger.del_read_watchpoint(addr),
            // Unsupported breakpoint type
            _ => return self.send_empty_reply(),
        }

        self.send_ok()
    }

    /// Write memory
    fn write_memory(&mut self, psx: &mut Psx, args: &[u8]) -> GdbResult {
        // Parse ADDR,LEN:DATA format
        let mut parts = args.split(|&b| b == b':');
        
        let addr_len = parts.next().ok_or(())?;
        let data = parts.next().ok_or(())?;
        
        let (addr, len) = parse_addr_len(addr_len)?;
        
        // Convert hex data to bytes
        let mut offset = 0;
        for i in 0..len {
            if data.len() < (i as usize + 1) * 2 {
                return self.send_error();
            }
            
            let h = ascii_hex(data[i as usize * 2]).ok_or(())?;
            let l = ascii_hex(data[i as usize * 2 + 1]).ok_or(())?;
            let byte = (h << 4) | l;
            
            psx.store::<u8>(addr.wrapping_add(offset), byte);
            offset += 1;
        }
        
        self.send_ok()
    }

    /// Write all registers
    fn write_registers(&mut self, psx: &mut Psx, args: &[u8]) -> GdbResult {
        // Each register is 8 hex chars (32 bits)
        if args.len() < 38 * 8 {
            return self.send_error();
        }
        
        // Parse general purpose registers (skip R0 which is always 0)
        for i in 1..32 {
            let offset = i * 8;
            let reg_hex = &args[offset..offset + 8];
            let value = parse_hex(reg_hex)?;
            psx.cpu.set_reg(RegisterIndex(i as u8), value);
        }
        
        // Parse special registers
        let sr = parse_hex(&args[32 * 8..33 * 8])?;
        let lo = parse_hex(&args[33 * 8..34 * 8])?;
        let hi = parse_hex(&args[34 * 8..35 * 8])?;
        let bad = parse_hex(&args[35 * 8..36 * 8])?;
        let cause = parse_hex(&args[36 * 8..37 * 8])?;
        let pc = parse_hex(&args[37 * 8..38 * 8])?;
        
        psx.cop0.set_sr(sr);
        psx.cpu.set_lo(lo);
        psx.cpu.set_hi(hi);
        psx.cop0.set_bad(bad);
        cop0::set_cause(psx, cause);
        psx.cpu.force_pc(pc);
        
        self.send_ok()
    }

    /// Write a single register
    fn write_register(&mut self, psx: &mut Psx, args: &[u8]) -> GdbResult {
        // Parse format: REGNUM=VALUE
        let mut parts = args.split(|&b| b == b'=');
        
        let reg_num = parts.next().ok_or(())?;
        let value_hex = parts.next().ok_or(())?;
        
        let reg_num = parse_hex(reg_num)?;
        let value = parse_hex(value_hex)?;
        
        match reg_num {
            0..=31 => {
                if reg_num != 0 {  // R0 is always 0
                    psx.cpu.set_reg(RegisterIndex(reg_num as u8), value);
                }
            }
            32 => psx.cop0.set_sr(value),
            33 => psx.cpu.set_lo(value),
            34 => psx.cpu.set_hi(value),
            35 => psx.cop0.set_bad(value),
            36 => cop0::set_cause(psx, value),
            37 => psx.cpu.force_pc(value),
            _ => return self.send_error(),
        }
        
        self.send_ok()
    }

    /// Handle query packets
    fn handle_query(&mut self, debugger: &mut Debugger, args: &[u8]) -> GdbResult {
        if args.starts_with(b"Supported") {
            // Report supported features
            self.send_string(b"PacketSize=1000;qXfer:features:read+;qXfer:threads:read+;QStartNoAckMode+;multiprocess+;swbreak+;hwbreak+")
        } else if args.starts_with(b"Attached") {
            // We're always attached
            self.send_string(b"1")
        } else if args.starts_with(b"fThreadInfo") {
            // Report single thread
            self.send_string(b"m1")
        } else if args.starts_with(b"sThreadInfo") {
            // No more threads
            self.send_string(b"l")
        } else if args.starts_with(b"ThreadExtraInfo") {
            // Thread info
            let mut reply = Reply::new();
            reply.push(b"PSX Main CPU");
            self.send_reply(reply)
        } else if args.starts_with(b"Symbol") {
            // Symbol lookup
            self.handle_symbol_query(debugger, args)
        } else if args.starts_with(b"Offsets") {
            // Report no offset
            self.send_string(b"Text=0;Data=0;Bss=0")
        } else {
            self.send_empty_reply()
        }
    }

    /// Handle set packets
    fn handle_set(&mut self, args: &[u8]) -> GdbResult {
        if args.starts_with(b"StartNoAckMode") {
            // Could enable no-ack mode here for better performance
            self.send_ok()
        } else {
            self.send_empty_reply()
        }
    }

    /// Kill the target
    fn kill(&mut self) -> GdbResult {
        // In an emulator context, we don't actually kill anything
        // Just acknowledge the command
        self.send_ok()
    }

    /// Detach from target
    fn detach(&mut self, debugger: &mut Debugger) -> GdbResult {
        // Clear all breakpoints and continue execution
        debugger.breakpoints.clear();
        debugger.read_watchpoints.clear();
        debugger.write_watchpoints.clear();
        debugger.resume();
        self.send_ok()
    }

    /// Set thread for subsequent operations
    fn set_thread(&mut self, _args: &[u8]) -> GdbResult {
        // We only have one thread
        self.send_ok()
    }

    /// Check if thread is alive
    fn thread_alive(&mut self, _args: &[u8]) -> GdbResult {
        // Our single thread is always alive
        self.send_ok()
    }

    /// Handle v packets (extended commands)
    fn handle_v_packet(&mut self, debugger: &mut Debugger, psx: &mut Psx, args: &[u8]) -> GdbResult {
        if args.starts_with(b"Cont?") {
            // Report supported continue actions
            self.send_string(b"vCont;c;s;C;S")
        } else if args.starts_with(b"Cont") {
            // Parse vCont commands
            let cmd = &args[4..];
            if cmd.starts_with(b";c") {
                // Continue
                self.resume(debugger, psx, &[])
            } else if cmd.starts_with(b";s") {
                // Step
                self.step(debugger, psx, &[])
            } else {
                self.send_error()
            }
        } else if args.starts_with(b"Kill") {
            self.kill()
        } else {
            self.send_empty_reply()
        }
    }

    /// Handle symbol queries
    fn handle_symbol_query(&mut self, debugger: &mut Debugger, args: &[u8]) -> GdbResult {
        // Parse qSymbol:[sym_value:]sym_name
        if args.starts_with(b"Symbol::") {
            // Initial query, just acknowledge
            self.send_ok()
        } else if args.starts_with(b"Symbol:") {
            // Symbol lookup request
            let data = &args[7..];
            
            // Find the colon separator if present (for value:name format)
            let parts: Vec<_> = data.split(|&b| b == b':').collect();
            
            if parts.len() == 2 {
                // We got a symbol value and name
                let _value = parts[0];
                let name_hex = parts[1];
                
                // Convert hex-encoded name to string
                let name = self.decode_hex_string(name_hex)?;
                
                // Store the symbol if needed
                info!("GDB provided symbol: {}", name);
            }
            
            self.send_ok()
        } else {
            self.send_empty_reply()
        }
    }

    /// Decode a hex-encoded string
    fn decode_hex_string(&self, hex: &[u8]) -> Result<String, ()> {
        let mut result = Vec::new();
        
        if hex.len() % 2 != 0 {
            return Err(());
        }
        
        for i in (0..hex.len()).step_by(2) {
            let h = ascii_hex(hex[i]).ok_or(())?;
            let l = ascii_hex(hex[i + 1]).ok_or(())?;
            result.push((h << 4) | l);
        }
        
        String::from_utf8(result).map_err(|_| ())
    }
}

enum PacketResult {
    Ok(Vec<u8>),
    BadChecksum(Vec<u8>),
    EndOfStream,
}

/// Get the value of an integer encoded in single lowercase hexadecimal ASCII digit. Return None if
/// the character is not valid hexadecimal
pub(super) fn ascii_hex(b: u8) -> Option<u8> {
    if b.is_ascii_digit() {
        Some(b - b'0')
    } else if b.is_ascii_hexdigit() {
        Some(10 + (b - b'a'))
    } else {
        // Invalid
        None
    }
}

/// Parse an hexadecimal string and return the value as an
/// integer. Return `None` if the string is invalid.
pub(super) fn parse_hex(hex: &[u8]) -> Result<u32, ()> {
    let mut v = 0u32;

    for &b in hex {
        v <<= 4;

        v |= match ascii_hex(b) {
            Some(h) => u32::from(h),
            // Bad hex
            None => return Err(()),
        };
    }

    Ok(v)
}

/// Parse a string in the format `addr,len` (both as hexadecimal
/// strings) and return the values as a tuple. Returns `None` if
/// the format is bogus.
pub(super) fn parse_addr_len(args: &[u8]) -> Result<(u32, u32), ()> {
    // split around the comma
    let args: Vec<_> = args.split(|&b| b == b',').collect();

    if args.len() != 2 {
        // Invalid number of arguments
        return Err(());
    }

    let addr = args[0];
    let len = args[1];

    if addr.is_empty() || len.is_empty() {
        // Missing parameter
        return Err(());
    }

    // Parse address
    let addr = parse_hex(addr)?;
    let len = parse_hex(len)?;

    Ok((addr, len))
}

/// Parse breakpoint arguments: the format is
/// `type,addr,kind`. Returns the three parameters in a tuple or an
/// error if a format error has been encountered.
pub(super) fn parse_breakpoint(args: &[u8]) -> Result<(u8, u32, u8), ()> {
    // split around the comma
    let args: Vec<_> = args.split(|&b| b == b',').collect();

    if args.len() != 3 {
        // Invalid number of arguments
        return Err(());
    }

    let btype = args[0];
    let addr = args[1];
    let kind = args[2];

    if btype.len() != 1 || kind.len() != 1 {
        // Type and kind should only be one character each
        return Err(());
    }

    let btype = btype[0];
    let kind = kind[0];

    let addr = parse_hex(addr)?;

    Ok((btype, addr, kind))
}
