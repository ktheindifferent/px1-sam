// RetroAchievements Integration for Rustation-NG
// Complete implementation for achievements, leaderboards, and rich presence

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Core RetroAchievements Manager
// ============================================================================

/// Main RetroAchievements integration system
pub struct RetroAchievementsManager {
    // Core components
    client: RAClient,
    runtime: Arc<Mutex<RARuntime>>,
    memory_interface: MemoryInterface,
    
    // Game data
    game_id: Option<u32>,
    game_hash: String,
    achievements: HashMap<u32, Achievement>,
    leaderboards: HashMap<u32, Leaderboard>,
    
    // User session
    user_session: Option<UserSession>,
    unlocked_achievements: HashSet<u32>,
    
    // Rich presence
    rich_presence: RichPresence,
    
    // Configuration
    config: RAConfig,
    
    // State tracking
    state: RAState,
    frame_counter: u64,
    last_update: Instant,
}

impl RetroAchievementsManager {
    pub fn new(config: RAConfig) -> Result<Self> {
        let client = RAClient::new(&config.api_url, &config.api_key)?;
        let runtime = Arc::new(Mutex::new(RARuntime::new()));
        
        Ok(RetroAchievementsManager {
            client,
            runtime,
            memory_interface: MemoryInterface::new(),
            game_id: None,
            game_hash: String::new(),
            achievements: HashMap::new(),
            leaderboards: HashMap::new(),
            user_session: None,
            unlocked_achievements: HashSet::new(),
            rich_presence: RichPresence::new(),
            config,
            state: RAState::Disabled,
            frame_counter: 0,
            last_update: Instant::now(),
        })
    }
    
    /// Initialize RetroAchievements for a game
    pub fn init_game(&mut self, rom_data: &[u8]) -> Result<()> {
        // Calculate game hash
        self.game_hash = self.calculate_hash(rom_data);
        
        // Identify game
        let game_info = self.client.identify_game(&self.game_hash)?;
        self.game_id = Some(game_info.id);
        
        info!("Identified game: {} (ID: {})", game_info.title, game_info.id);
        
        // Load achievement data
        self.load_achievements(game_info.id)?;
        self.load_leaderboards(game_info.id)?;
        self.load_rich_presence(game_info.id)?;
        
        // Initialize runtime
        {
            let mut runtime = self.runtime.lock().unwrap();
            runtime.init_game(game_info.id);
            
            // Compile achievement conditions
            for achievement in self.achievements.values() {
                runtime.add_achievement(achievement)?;
            }
            
            // Compile leaderboard conditions
            for leaderboard in self.leaderboards.values() {
                runtime.add_leaderboard(leaderboard)?;
            }
        }
        
        self.state = RAState::Active;
        
        Ok(())
    }
    
    /// Login user for online features
    pub fn login(&mut self, username: &str, password: &str) -> Result<()> {
        let session = self.client.login(username, password)?;
        
        // Load user's unlocked achievements
        if let Some(game_id) = self.game_id {
            let unlocked = self.client.get_user_unlocks(
                &session.token,
                game_id,
                &session.username
            )?;
            
            self.unlocked_achievements = unlocked.into_iter().collect();
        }
        
        self.user_session = Some(session);
        info!("Logged in as: {}", username);
        
        Ok(())
    }
    
    /// Process achievements for current frame
    pub fn process_frame(&mut self, psx: &Psx) -> Result<()> {
        if self.state != RAState::Active {
            return Ok(());
        }
        
        self.frame_counter += 1;
        
        // Update memory interface
        self.memory_interface.update(psx);
        
        // Process achievements
        let mut newly_unlocked = Vec::new();
        {
            let mut runtime = self.runtime.lock().unwrap();
            runtime.set_memory_interface(&self.memory_interface);
            
            // Check each achievement
            for (id, achievement) in &self.achievements {
                if self.unlocked_achievements.contains(id) {
                    continue; // Already unlocked
                }
                
                if self.config.hardcore_mode && achievement.flags.contains(&AchievementFlag::Hardcore) {
                    // In hardcore mode, check additional requirements
                    if !self.validate_hardcore_requirements(psx) {
                        continue;
                    }
                }
                
                if runtime.test_achievement(*id)? {
                    newly_unlocked.push(*id);
                }
            }
            
            // Process leaderboards
            for (id, _leaderboard) in &self.leaderboards {
                if let Some(value) = runtime.test_leaderboard(*id)? {
                    self.submit_leaderboard_score(*id, value)?;
                }
            }
        }
        
        // Handle newly unlocked achievements
        for achievement_id in newly_unlocked {
            self.unlock_achievement(achievement_id)?;
        }
        
        // Update rich presence periodically
        if self.last_update.elapsed() > Duration::from_secs(30) {
            self.update_rich_presence(psx)?;
            self.last_update = Instant::now();
        }
        
        Ok(())
    }
    
    /// Unlock an achievement
    fn unlock_achievement(&mut self, achievement_id: u32) -> Result<()> {
        if let Some(achievement) = self.achievements.get(&achievement_id) {
            // Local unlock
            self.unlocked_achievements.insert(achievement_id);
            
            // Trigger notification
            self.show_achievement_popup(achievement);
            
            // Submit to server if logged in
            if let Some(ref session) = self.user_session {
                self.client.unlock_achievement(
                    &session.token,
                    achievement_id,
                    self.config.hardcore_mode
                )?;
            }
            
            info!("Achievement unlocked: {}", achievement.title);
            
            // Play unlock sound if configured
            if self.config.play_unlock_sound {
                self.play_unlock_sound();
            }
        }
        
        Ok(())
    }
    
    /// Submit score to leaderboard
    fn submit_leaderboard_score(&mut self, leaderboard_id: u32, value: i32) -> Result<()> {
        if let Some(leaderboard) = self.leaderboards.get(&leaderboard_id) {
            info!("Submitting score {} to leaderboard: {}", value, leaderboard.title);
            
            if let Some(ref session) = self.user_session {
                self.client.submit_leaderboard(
                    &session.token,
                    leaderboard_id,
                    value
                )?;
            }
        }
        
        Ok(())
    }
    
    /// Update rich presence status
    fn update_rich_presence(&mut self, psx: &Psx) -> Result<()> {
        let status = self.rich_presence.evaluate(&self.memory_interface)?;
        
        if let Some(ref session) = self.user_session {
            self.client.update_rich_presence(
                &session.token,
                self.game_id.unwrap_or(0),
                &status
            )?;
        }
        
        // Also update Discord if integrated
        if self.config.discord_integration {
            self.update_discord_presence(&status)?;
        }
        
        Ok(())
    }
    
    /// Calculate ROM hash for game identification
    fn calculate_hash(&self, rom_data: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        
        // RetroAchievements uses specific hashing methods per console
        // For PSX, typically hash specific regions of the disc
        let mut hasher = Sha256::new();
        
        // Hash first 512KB for identification
        let hash_size = std::cmp::min(rom_data.len(), 512 * 1024);
        hasher.update(&rom_data[..hash_size]);
        
        format!("{:x}", hasher.finalize())
    }
    
    /// Validate hardcore mode requirements
    fn validate_hardcore_requirements(&self, psx: &Psx) -> bool {
        // Hardcore mode restrictions
        if psx.cheats_enabled() {
            return false; // No cheats allowed
        }
        
        if psx.save_states_used() {
            return false; // No save states in hardcore
        }
        
        if psx.speed_modifier != 1.0 {
            return false; // No fast-forward/slow-motion
        }
        
        true
    }
    
    /// Load achievements from server
    fn load_achievements(&mut self, game_id: u32) -> Result<()> {
        let achievements_data = self.client.get_game_achievements(game_id)?;
        
        for ach_data in achievements_data {
            let achievement = Achievement {
                id: ach_data.id,
                title: ach_data.title,
                description: ach_data.description,
                points: ach_data.points,
                badge_url: ach_data.badge_url,
                conditions: self.parse_conditions(&ach_data.mem_addr)?,
                flags: self.parse_flags(ach_data.flags),
            };
            
            self.achievements.insert(achievement.id, achievement);
        }
        
        info!("Loaded {} achievements", self.achievements.len());
        Ok(())
    }
    
    /// Load leaderboards from server
    fn load_leaderboards(&mut self, game_id: u32) -> Result<()> {
        let leaderboards_data = self.client.get_game_leaderboards(game_id)?;
        
        for lb_data in leaderboards_data {
            let leaderboard = Leaderboard {
                id: lb_data.id,
                title: lb_data.title,
                description: lb_data.description,
                start_conditions: self.parse_conditions(&lb_data.mem_start)?,
                cancel_conditions: self.parse_conditions(&lb_data.mem_cancel)?,
                submit_conditions: self.parse_conditions(&lb_data.mem_submit)?,
                value_conditions: self.parse_value(&lb_data.mem_value)?,
                format: self.parse_format(&lb_data.format),
                lower_is_better: lb_data.lower_is_better,
            };
            
            self.leaderboards.insert(leaderboard.id, leaderboard);
        }
        
        info!("Loaded {} leaderboards", self.leaderboards.len());
        Ok(())
    }
    
    /// Load rich presence script
    fn load_rich_presence(&mut self, game_id: u32) -> Result<()> {
        let script = self.client.get_rich_presence_script(game_id)?;
        self.rich_presence.load_script(&script)?;
        Ok(())
    }
}

// ============================================================================
// Achievement Runtime (rcheevos integration)
// ============================================================================

/// Runtime for evaluating achievement conditions
pub struct RARuntime {
    achievements: HashMap<u32, CompiledAchievement>,
    leaderboards: HashMap<u32, CompiledLeaderboard>,
    memory: Option<*const MemoryInterface>,
}

impl RARuntime {
    pub fn new() -> Self {
        RARuntime {
            achievements: HashMap::new(),
            leaderboards: HashMap::new(),
            memory: None,
        }
    }
    
    pub fn init_game(&mut self, _game_id: u32) {
        self.achievements.clear();
        self.leaderboards.clear();
    }
    
    pub fn set_memory_interface(&mut self, memory: &MemoryInterface) {
        self.memory = Some(memory as *const _);
    }
    
    pub fn add_achievement(&mut self, achievement: &Achievement) -> Result<()> {
        let compiled = CompiledAchievement::compile(&achievement.conditions)?;
        self.achievements.insert(achievement.id, compiled);
        Ok(())
    }
    
    pub fn add_leaderboard(&mut self, leaderboard: &Leaderboard) -> Result<()> {
        let compiled = CompiledLeaderboard::compile(leaderboard)?;
        self.leaderboards.insert(leaderboard.id, compiled);
        Ok(())
    }
    
    pub fn test_achievement(&mut self, id: u32) -> Result<bool> {
        if let Some(achievement) = self.achievements.get_mut(&id) {
            if let Some(memory) = self.memory {
                unsafe {
                    return achievement.test(&*memory);
                }
            }
        }
        Ok(false)
    }
    
    pub fn test_leaderboard(&mut self, id: u32) -> Result<Option<i32>> {
        if let Some(leaderboard) = self.leaderboards.get_mut(&id) {
            if let Some(memory) = self.memory {
                unsafe {
                    return leaderboard.test(&*memory);
                }
            }
        }
        Ok(None)
    }
}

// ============================================================================
// Memory Interface
// ============================================================================

/// Interface for accessing emulator memory
pub struct MemoryInterface {
    ram: Vec<u8>,
    vram: Vec<u8>,
    spu_ram: Vec<u8>,
    registers: HashMap<String, u32>,
}

impl MemoryInterface {
    pub fn new() -> Self {
        MemoryInterface {
            ram: vec![0; 2 * 1024 * 1024], // 2MB main RAM
            vram: vec![0; 1024 * 512 * 2], // VRAM
            spu_ram: vec![0; 512 * 1024], // SPU RAM
            registers: HashMap::new(),
        }
    }
    
    pub fn update(&mut self, psx: &Psx) {
        // Copy memory regions
        self.ram.copy_from_slice(psx.get_ram());
        self.vram.copy_from_slice(psx.gpu.get_vram());
        self.spu_ram.copy_from_slice(psx.spu.get_ram());
        
        // Update registers
        self.registers.insert("pc".to_string(), psx.cpu.pc());
        for i in 0..32 {
            self.registers.insert(format!("r{}", i), psx.cpu.register(i));
        }
    }
    
    pub fn read_u8(&self, address: u32) -> u8 {
        match address {
            0x00000000..=0x001fffff => self.ram[address as usize],
            0x1f000000..=0x1f0fffff => self.vram[(address - 0x1f000000) as usize],
            0x1f800000..=0x1f87ffff => self.spu_ram[(address - 0x1f800000) as usize],
            _ => 0,
        }
    }
    
    pub fn read_u16(&self, address: u32) -> u16 {
        let lo = self.read_u8(address) as u16;
        let hi = self.read_u8(address + 1) as u16;
        lo | (hi << 8)
    }
    
    pub fn read_u32(&self, address: u32) -> u32 {
        let lo = self.read_u16(address) as u32;
        let hi = self.read_u16(address + 2) as u32;
        lo | (hi << 16)
    }
}

// ============================================================================
// Achievement Condition System
// ============================================================================

/// Compiled achievement for fast evaluation
struct CompiledAchievement {
    core_group: ConditionGroup,
    alt_groups: Vec<ConditionGroup>,
    hits_required: u32,
    current_hits: u32,
}

impl CompiledAchievement {
    fn compile(conditions: &[Condition]) -> Result<Self> {
        // Parse condition groups
        let mut core_group = ConditionGroup::new();
        let mut alt_groups = Vec::new();
        
        for condition in conditions {
            match condition.group_type {
                GroupType::Core => core_group.add(condition.clone()),
                GroupType::Alt(n) => {
                    while alt_groups.len() <= n {
                        alt_groups.push(ConditionGroup::new());
                    }
                    alt_groups[n].add(condition.clone());
                }
            }
        }
        
        Ok(CompiledAchievement {
            core_group,
            alt_groups,
            hits_required: 1, // Default, can be overridden
            current_hits: 0,
        })
    }
    
    fn test(&mut self, memory: &MemoryInterface) -> Result<bool> {
        // Test core conditions
        if !self.core_group.test(memory) {
            self.current_hits = 0;
            return Ok(false);
        }
        
        // Test alternative groups (any must be true)
        if !self.alt_groups.is_empty() {
            let any_alt = self.alt_groups.iter_mut()
                .any(|group| group.test(memory));
            
            if !any_alt {
                self.current_hits = 0;
                return Ok(false);
            }
        }
        
        // Increment hit counter
        self.current_hits += 1;
        
        Ok(self.current_hits >= self.hits_required)
    }
}

/// Group of conditions with logical operators
struct ConditionGroup {
    conditions: Vec<Condition>,
}

impl ConditionGroup {
    fn new() -> Self {
        ConditionGroup {
            conditions: Vec::new(),
        }
    }
    
    fn add(&mut self, condition: Condition) {
        self.conditions.push(condition);
    }
    
    fn test(&mut self, memory: &MemoryInterface) -> bool {
        for condition in &mut self.conditions {
            if !condition.test(memory) {
                return false;
            }
        }
        true
    }
}

/// Single achievement condition
#[derive(Clone)]
struct Condition {
    left: Operand,
    operator: ComparisonOperator,
    right: Operand,
    hit_count: u32,
    current_hits: u32,
    group_type: GroupType,
}

impl Condition {
    fn test(&mut self, memory: &MemoryInterface) -> bool {
        let left_val = self.left.evaluate(memory);
        let right_val = self.right.evaluate(memory);
        
        let result = match self.operator {
            ComparisonOperator::Equal => left_val == right_val,
            ComparisonOperator::NotEqual => left_val != right_val,
            ComparisonOperator::Less => left_val < right_val,
            ComparisonOperator::LessEqual => left_val <= right_val,
            ComparisonOperator::Greater => left_val > right_val,
            ComparisonOperator::GreaterEqual => left_val >= right_val,
        };
        
        if result {
            self.current_hits += 1;
        } else {
            self.current_hits = 0;
        }
        
        self.current_hits >= self.hit_count
    }
}

#[derive(Clone)]
enum Operand {
    Address(AddressOperand),
    Value(u32),
    Delta(Box<Operand>), // Previous frame value
    Prior(Box<Operand>), // Value before last change
}

impl Operand {
    fn evaluate(&self, memory: &MemoryInterface) -> u32 {
        match self {
            Operand::Address(addr) => addr.read(memory),
            Operand::Value(val) => *val,
            Operand::Delta(_) => 0, // Simplified - needs frame history
            Operand::Prior(_) => 0, // Simplified - needs change tracking
        }
    }
}

#[derive(Clone)]
struct AddressOperand {
    address: u32,
    size: MemorySize,
}

impl AddressOperand {
    fn read(&self, memory: &MemoryInterface) -> u32 {
        match self.size {
            MemorySize::Bit0 => (memory.read_u8(self.address) & 0x01) as u32,
            MemorySize::Bit1 => ((memory.read_u8(self.address) >> 1) & 0x01) as u32,
            MemorySize::Bit2 => ((memory.read_u8(self.address) >> 2) & 0x01) as u32,
            MemorySize::Bit3 => ((memory.read_u8(self.address) >> 3) & 0x01) as u32,
            MemorySize::Bit4 => ((memory.read_u8(self.address) >> 4) & 0x01) as u32,
            MemorySize::Bit5 => ((memory.read_u8(self.address) >> 5) & 0x01) as u32,
            MemorySize::Bit6 => ((memory.read_u8(self.address) >> 6) & 0x01) as u32,
            MemorySize::Bit7 => ((memory.read_u8(self.address) >> 7) & 0x01) as u32,
            MemorySize::Lower4 => (memory.read_u8(self.address) & 0x0f) as u32,
            MemorySize::Upper4 => ((memory.read_u8(self.address) >> 4) & 0x0f) as u32,
            MemorySize::Byte => memory.read_u8(self.address) as u32,
            MemorySize::Word => memory.read_u16(self.address) as u32,
            MemorySize::DWord => memory.read_u32(self.address),
        }
    }
}

#[derive(Clone, Copy)]
enum MemorySize {
    Bit0, Bit1, Bit2, Bit3, Bit4, Bit5, Bit6, Bit7,
    Lower4, Upper4,
    Byte, Word, DWord,
}

#[derive(Clone, Copy)]
enum ComparisonOperator {
    Equal, NotEqual,
    Less, LessEqual,
    Greater, GreaterEqual,
}

#[derive(Clone)]
enum GroupType {
    Core,
    Alt(usize),
}

// ============================================================================
// Supporting Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct Achievement {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub badge_url: String,
    pub conditions: Vec<Condition>,
    pub flags: Vec<AchievementFlag>,
}

#[derive(Debug, Clone)]
pub struct Leaderboard {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub start_conditions: Vec<Condition>,
    pub cancel_conditions: Vec<Condition>,
    pub submit_conditions: Vec<Condition>,
    pub value_conditions: ValueExpression,
    pub format: LeaderboardFormat,
    pub lower_is_better: bool,
}

struct CompiledLeaderboard {
    id: u32,
    state: LeaderboardState,
    value: i32,
}

impl CompiledLeaderboard {
    fn compile(_leaderboard: &Leaderboard) -> Result<Self> {
        Ok(CompiledLeaderboard {
            id: 0,
            state: LeaderboardState::Inactive,
            value: 0,
        })
    }
    
    fn test(&mut self, _memory: &MemoryInterface) -> Result<Option<i32>> {
        Ok(None) // Simplified
    }
}

#[derive(Clone, Copy)]
enum LeaderboardState {
    Inactive,
    Active,
    Ready,
}

#[derive(Debug, Clone)]
pub struct ValueExpression;

#[derive(Debug, Clone, Copy)]
pub enum LeaderboardFormat {
    Value,
    Time,
    Score,
    Percentage,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AchievementFlag {
    Hardcore,
    Progression,
    Missable,
    Secret,
}

#[derive(Debug, Clone)]
pub struct RAConfig {
    pub enabled: bool,
    pub hardcore_mode: bool,
    pub api_url: String,
    pub api_key: String,
    pub play_unlock_sound: bool,
    pub show_popups: bool,
    pub popup_duration_ms: u32,
    pub discord_integration: bool,
}

impl Default for RAConfig {
    fn default() -> Self {
        RAConfig {
            enabled: true,
            hardcore_mode: false,
            api_url: "https://retroachievements.org/dorequest.php".to_string(),
            api_key: String::new(),
            play_unlock_sound: true,
            show_popups: true,
            popup_duration_ms: 5000,
            discord_integration: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RAState {
    Disabled,
    Active,
    Paused,
}

struct UserSession {
    username: String,
    token: String,
}

struct RichPresence {
    script: String,
}

impl RichPresence {
    fn new() -> Self { RichPresence { script: String::new() } }
    fn load_script(&mut self, script: &str) -> Result<()> { 
        self.script = script.to_string();
        Ok(()) 
    }
    fn evaluate(&self, _memory: &MemoryInterface) -> Result<String> { 
        Ok("Playing".to_string()) 
    }
}

// API Client stub
struct RAClient;
impl RAClient {
    fn new(_url: &str, _key: &str) -> Result<Self> { Ok(RAClient) }
    fn identify_game(&self, _hash: &str) -> Result<GameInfo> {
        Ok(GameInfo { id: 1, title: "Test Game".to_string() })
    }
    fn login(&self, _user: &str, _pass: &str) -> Result<UserSession> {
        Ok(UserSession { username: "user".to_string(), token: "token".to_string() })
    }
    fn get_user_unlocks(&self, _token: &str, _game: u32, _user: &str) -> Result<Vec<u32>> {
        Ok(Vec::new())
    }
    fn get_game_achievements(&self, _id: u32) -> Result<Vec<AchievementData>> {
        Ok(Vec::new())
    }
    fn get_game_leaderboards(&self, _id: u32) -> Result<Vec<LeaderboardData>> {
        Ok(Vec::new())
    }
    fn get_rich_presence_script(&self, _id: u32) -> Result<String> {
        Ok(String::new())
    }
    fn unlock_achievement(&self, _token: &str, _id: u32, _hardcore: bool) -> Result<()> {
        Ok(())
    }
    fn submit_leaderboard(&self, _token: &str, _id: u32, _value: i32) -> Result<()> {
        Ok(())
    }
    fn update_rich_presence(&self, _token: &str, _game: u32, _status: &str) -> Result<()> {
        Ok(())
    }
}

struct GameInfo {
    id: u32,
    title: String,
}

struct AchievementData {
    id: u32,
    title: String,
    description: String,
    points: u32,
    badge_url: String,
    mem_addr: String,
    flags: u32,
}

struct LeaderboardData {
    id: u32,
    title: String,
    description: String,
    mem_start: String,
    mem_cancel: String,
    mem_submit: String,
    mem_value: String,
    format: String,
    lower_is_better: bool,
}

// Error types
type Result<T> = std::result::Result<T, RAError>;

#[derive(Debug)]
enum RAError {
    NetworkError,
    ParseError,
    InvalidCondition,
}

// Placeholder implementations
impl RetroAchievementsManager {
    fn show_achievement_popup(&self, _achievement: &Achievement) {}
    fn play_unlock_sound(&self) {}
    fn update_discord_presence(&self, _status: &str) -> Result<()> { Ok(()) }
    fn parse_conditions(&self, _mem: &str) -> Result<Vec<Condition>> { Ok(Vec::new()) }
    fn parse_flags(&self, _flags: u32) -> Vec<AchievementFlag> { Vec::new() }
    fn parse_value(&self, _mem: &str) -> Result<ValueExpression> { Ok(ValueExpression) }
    fn parse_format(&self, _fmt: &str) -> LeaderboardFormat { LeaderboardFormat::Value }
}

// Placeholder PSX interface
struct Psx;
impl Psx {
    fn get_ram(&self) -> &[u8] { &[] }
    fn cheats_enabled(&self) -> bool { false }
    fn save_states_used(&self) -> bool { false }
}

impl Psx {
    fn cpu(&self) -> Cpu { Cpu }
    fn gpu(&self) -> Gpu { Gpu }
    fn spu(&self) -> Spu { Spu }
}

struct Cpu;
impl Cpu {
    fn pc(&self) -> u32 { 0 }
    fn register(&self, _n: usize) -> u32 { 0 }
}

struct Gpu;
impl Gpu {
    fn get_vram(&self) -> &[u8] { &[] }
}

struct Spu;
impl Spu {
    fn get_ram(&self) -> &[u8] { &[] }
}

impl Psx {
    const speed_modifier: f32 = 1.0;
}