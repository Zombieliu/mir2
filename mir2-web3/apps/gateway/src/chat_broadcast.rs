use std::collections::HashMap;
use std::env;
use std::future::pending;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_protocol::{ChatType, ServerPacket};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

const ONLINE_INTERVAL: Duration = Duration::from_secs(5 * 60);
const LINE_MESSAGE_INTERVAL: Duration = Duration::from_secs(10 * 60);
const BROADCAST_CAPACITY: usize = 64;
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(1);

const LINE_MESSAGE_PATH_ENV: &str = "MIR2_GATEWAY_LINE_MESSAGE_PATH";
const QA_CONTROL_TOKEN_ENV: &str = "MIR2_GATEWAY_QA_CONTROL_TOKEN";
const QA_ONLINE_INTERVAL_MS_ENV: &str = "MIR2_GATEWAY_QA_CHAT_ONLINE_INTERVAL_MS";
const QA_LINE_INTERVAL_MS_ENV: &str = "MIR2_GATEWAY_QA_CHAT_LINE_INTERVAL_MS";
const QA_FIXED_LINE_INDEX_ENV: &str = "MIR2_GATEWAY_QA_CHAT_FIXED_LINE_INDEX";
const QA_MAX_PACKETS_ENV: &str = "MIR2_GATEWAY_QA_CHAT_MAX_PACKETS";

const DEFAULT_LINE_MESSAGES: &[&str] = &[
    "Welcome to Crystal Mir 2 released by Suprcode.",
    "Make sure to follow JevLomcn on github for the latest Database releases.",
    "www.LOMCN.net",
    "Now in Net.8",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatProtocol {
    Tcp,
    WebSocket,
}

#[derive(Debug, Clone)]
struct ChatBroadcastConfig {
    online_interval: Duration,
    line_message_interval: Duration,
    fixed_line_index: Option<usize>,
    max_packets: Option<usize>,
}

impl Default for ChatBroadcastConfig {
    fn default() -> Self {
        Self {
            online_interval: ONLINE_INTERVAL,
            line_message_interval: LINE_MESSAGE_INTERVAL,
            fixed_line_index: None,
            max_packets: None,
        }
    }
}

impl ChatBroadcastConfig {
    fn from_env() -> Self {
        Self::from_lookup(|name| env::var(name).ok())
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        let qa_enabled = lookup(QA_CONTROL_TOKEN_ENV).is_some_and(|token| !token.trim().is_empty());
        if !qa_enabled {
            return Self::default();
        }

        Self {
            online_interval: duration_from_millis(
                lookup(QA_ONLINE_INTERVAL_MS_ENV),
                ONLINE_INTERVAL,
            ),
            line_message_interval: duration_from_millis(
                lookup(QA_LINE_INTERVAL_MS_ENV),
                LINE_MESSAGE_INTERVAL,
            ),
            fixed_line_index: lookup(QA_FIXED_LINE_INDEX_ENV)
                .and_then(|value| value.trim().parse::<usize>().ok()),
            max_packets: lookup(QA_MAX_PACKETS_ENV)
                .and_then(|value| value.trim().parse::<usize>().ok())
                .filter(|value| *value > 0),
        }
    }

    fn poll_interval(&self) -> Duration {
        self.online_interval
            .min(self.line_message_interval)
            .min(MAX_POLL_INTERVAL)
            .max(Duration::from_millis(1))
    }
}

fn duration_from_millis(value: Option<String>, fallback: Duration) -> Duration {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|milliseconds| *milliseconds > 0)
        .map(Duration::from_millis)
        .unwrap_or(fallback)
}

#[derive(Debug)]
struct PresenceState {
    next_id: u64,
    protocols: HashMap<u64, ChatProtocol>,
}

impl Default for PresenceState {
    fn default() -> Self {
        Self {
            next_id: 1,
            protocols: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct ChatBroadcastInner {
    presence: Mutex<PresenceState>,
    sender: broadcast::Sender<ServerPacket>,
}

#[derive(Clone, Debug)]
pub struct ChatBroadcastHub {
    inner: Arc<ChatBroadcastInner>,
    config: Arc<ChatBroadcastConfig>,
    line_messages: Arc<Vec<String>>,
}

impl ChatBroadcastHub {
    pub fn from_env() -> io::Result<Self> {
        let config = ChatBroadcastConfig::from_env();
        let line_messages = load_line_messages_from_env()?;
        if let Some(index) = config.fixed_line_index {
            if index >= line_messages.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "{QA_FIXED_LINE_INDEX_ENV} index {index} exceeds {} configured line messages",
                        line_messages.len()
                    ),
                ));
            }
        }
        Ok(Self::new(config, line_messages))
    }

    fn new(config: ChatBroadcastConfig, line_messages: Vec<String>) -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(ChatBroadcastInner {
                presence: Mutex::new(PresenceState::default()),
                sender,
            }),
            config: Arc::new(config),
            line_messages: Arc::new(if line_messages.is_empty() {
                default_line_messages()
            } else {
                line_messages
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::new(ChatBroadcastConfig::default(), default_line_messages())
    }

    pub fn register(&self, protocol: ChatProtocol) -> ChatPresence {
        let mut presence = self
            .inner
            .presence
            .lock()
            .expect("chat presence mutex should not be poisoned");
        let receiver = self.inner.sender.subscribe();
        let id = presence.next_id;
        presence.next_id = presence.next_id.wrapping_add(1).max(1);
        presence.protocols.insert(id, protocol);
        ChatPresence {
            inner: Some(Arc::clone(&self.inner)),
            id,
            receiver,
        }
    }

    pub fn online_count(&self) -> usize {
        self.inner
            .presence
            .lock()
            .expect("chat presence mutex should not be poisoned")
            .protocols
            .len()
    }

    pub fn spawn(&self) -> ChatBroadcastTask {
        let poll_interval = self.config.poll_interval();
        let mut scheduler =
            self.scheduler_with_clock(Arc::new(SystemChatClock::new()), production_random_seed());
        let handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(poll_interval);
            tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                scheduler.tick();
            }
        });
        ChatBroadcastTask { handle }
    }

    pub fn scheduler_with_clock(
        &self,
        clock: Arc<dyn ChatClock>,
        random_seed: u64,
    ) -> ChatBroadcastScheduler {
        ChatBroadcastScheduler {
            hub: self.clone(),
            clock,
            schedule: ChatSchedule::new(&self.config),
            random: XorShift64::new(random_seed),
            published_packets: 0,
        }
    }

    fn publish(&self, packets: &[ServerPacket]) {
        for packet in packets {
            let _ = self.inner.sender.send(packet.clone());
        }
    }
}

pub struct ChatPresence {
    inner: Option<Arc<ChatBroadcastInner>>,
    id: u64,
    receiver: broadcast::Receiver<ServerPacket>,
}

impl ChatPresence {
    pub async fn recv(&mut self) -> Result<ServerPacket, broadcast::error::RecvError> {
        self.receiver.recv().await
    }
}

impl Drop for ChatPresence {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        inner
            .presence
            .lock()
            .expect("chat presence mutex should not be poisoned")
            .protocols
            .remove(&self.id);
    }
}

pub async fn recv_optional_chat(
    presence: &mut Option<ChatPresence>,
) -> Result<ServerPacket, broadcast::error::RecvError> {
    match presence.as_mut() {
        Some(presence) => presence.recv().await,
        None => pending().await,
    }
}

pub trait ChatClock: Send + Sync {
    fn elapsed(&self) -> Duration;
}

#[derive(Debug)]
struct SystemChatClock {
    started_at: Instant,
}

impl SystemChatClock {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl ChatClock for SystemChatClock {
    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

pub struct ChatBroadcastScheduler {
    hub: ChatBroadcastHub,
    clock: Arc<dyn ChatClock>,
    schedule: ChatSchedule,
    random: XorShift64,
    published_packets: usize,
}

impl ChatBroadcastScheduler {
    pub fn tick(&mut self) -> Vec<ServerPacket> {
        let online_count = self.hub.online_count();
        if self.hub.config.max_packets.is_some() && online_count == 0 {
            return Vec::new();
        }
        let remaining = self
            .hub
            .config
            .max_packets
            .map(|limit| limit.saturating_sub(self.published_packets));
        if remaining == Some(0) {
            return Vec::new();
        }
        let packets = self.schedule.due_packets(
            self.clock.elapsed(),
            online_count,
            &self.hub.line_messages,
            self.hub.config.fixed_line_index,
            remaining,
            &mut self.random,
        );
        self.published_packets += packets.len();
        self.hub.publish(&packets);
        packets
    }
}

pub struct ChatBroadcastTask {
    handle: JoinHandle<()>,
}

impl Drop for ChatBroadcastTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[derive(Debug)]
struct ChatSchedule {
    online_interval: Duration,
    line_message_interval: Duration,
    next_online: Duration,
    next_line_message: Duration,
}

impl ChatSchedule {
    fn new(config: &ChatBroadcastConfig) -> Self {
        Self {
            online_interval: config.online_interval,
            line_message_interval: config.line_message_interval,
            next_online: config.online_interval,
            next_line_message: config.line_message_interval,
        }
    }

    fn due_packets(
        &mut self,
        now: Duration,
        online_count: usize,
        line_messages: &[String],
        fixed_line_index: Option<usize>,
        max_packets: Option<usize>,
        random: &mut XorShift64,
    ) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        while self.next_online <= now || self.next_line_message <= now {
            if max_packets.is_some_and(|limit| packets.len() >= limit) {
                break;
            }
            if self.next_online <= self.next_line_message && self.next_online <= now {
                packets.push(ServerPacket::Chat {
                    message: format!("Online Players: {online_count}"),
                    chat_type: ChatType::Hint,
                });
                self.next_online = advance(self.next_online, self.online_interval);
                continue;
            }

            if self.next_line_message <= now {
                let index =
                    fixed_line_index.unwrap_or_else(|| random.next_index(line_messages.len()));
                packets.push(ServerPacket::Chat {
                    message: line_messages[index].clone(),
                    chat_type: ChatType::LineMessage,
                });
                self.next_line_message =
                    advance(self.next_line_message, self.line_message_interval);
            }
        }
        packets
    }
}

fn advance(current: Duration, interval: Duration) -> Duration {
    current.checked_add(interval).unwrap_or(Duration::MAX)
}

#[derive(Debug)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn next_index(&mut self, len: usize) -> usize {
        debug_assert!(len > 0);
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as usize) % len
    }
}

fn production_random_seed() -> u64 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    time ^ u64::from(std::process::id()).rotate_left(23)
}

fn load_line_messages_from_env() -> io::Result<Vec<String>> {
    if let Some(path) = env::var_os(LINE_MESSAGE_PATH_ENV).filter(|path| !path.is_empty()) {
        return load_line_messages(&PathBuf::from(path)).and_then(require_line_messages);
    }

    let candidates = [
        PathBuf::from("Envir/LineMessage.txt"),
        PathBuf::from("Build/Server/Debug/Envir/LineMessage.txt"),
        PathBuf::from("../Crystal/Build/Server/Debug/Envir/LineMessage.txt"),
    ];
    for path in candidates {
        if path.is_file() {
            return load_line_messages(&path).and_then(require_line_messages);
        }
    }
    Ok(default_line_messages())
}

fn load_line_messages(path: &Path) -> io::Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

fn require_line_messages(messages: Vec<String>) -> io::Result<Vec<String>> {
    if messages.is_empty() {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LineMessage.txt contains no announcements",
        ))
    } else {
        Ok(messages)
    }
}

fn default_line_messages() -> Vec<String> {
    DEFAULT_LINE_MESSAGES
        .iter()
        .map(|message| (*message).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    #[derive(Default)]
    struct ManualClock {
        elapsed_ms: AtomicU64,
    }

    impl ManualClock {
        fn set_minutes(&self, minutes: u64) {
            self.elapsed_ms
                .store(minutes * 60 * 1_000, Ordering::Release);
        }
    }

    impl ChatClock for ManualClock {
        fn elapsed(&self) -> Duration {
            Duration::from_millis(self.elapsed_ms.load(Ordering::Acquire))
        }
    }

    fn deterministic_hub() -> ChatBroadcastHub {
        ChatBroadcastHub::new(
            ChatBroadcastConfig {
                fixed_line_index: Some(1),
                ..ChatBroadcastConfig::default()
            },
            vec!["first line".to_string(), "fixed line".to_string()],
        )
    }

    fn assert_online(packet: &ServerPacket, count: usize) {
        assert_eq!(
            packet,
            &ServerPacket::Chat {
                message: format!("Online Players: {count}"),
                chat_type: ChatType::Hint,
            }
        );
    }

    fn assert_line_message(packet: &ServerPacket) {
        assert_eq!(
            packet,
            &ServerPacket::Chat {
                message: "fixed line".to_string(),
                chat_type: ChatType::LineMessage,
            }
        );
    }

    #[test]
    fn schedule_orders_zero_five_ten_fifteen_twenty_minutes() {
        let hub = deterministic_hub();
        let _presence = hub.register(ChatProtocol::Tcp);
        let clock = Arc::new(ManualClock::default());
        let mut scheduler = hub.scheduler_with_clock(clock.clone(), 7);

        assert!(scheduler.tick().is_empty());

        clock.set_minutes(5);
        let packets = scheduler.tick();
        assert_eq!(packets.len(), 1);
        assert_online(&packets[0], 1);

        clock.set_minutes(10);
        let packets = scheduler.tick();
        assert_eq!(packets.len(), 2);
        assert_online(&packets[0], 1);
        assert_line_message(&packets[1]);

        clock.set_minutes(15);
        let packets = scheduler.tick();
        assert_eq!(packets.len(), 1);
        assert_online(&packets[0], 1);

        clock.set_minutes(20);
        let packets = scheduler.tick();
        assert_eq!(packets.len(), 2);
        assert_online(&packets[0], 1);
        assert_line_message(&packets[1]);
    }

    #[tokio::test]
    async fn tcp_and_websocket_share_presence_and_broadcasts() {
        let hub = deterministic_hub();
        let mut tcp = hub.register(ChatProtocol::Tcp);
        let mut web = hub.register(ChatProtocol::WebSocket);
        assert_eq!(hub.online_count(), 2);

        let clock = Arc::new(ManualClock::default());
        let mut scheduler = hub.scheduler_with_clock(clock.clone(), 11);
        clock.set_minutes(5);
        scheduler.tick();

        assert_online(&tcp.recv().await.expect("TCP should receive broadcast"), 2);
        assert_online(
            &web.recv()
                .await
                .expect("WebSocket should receive broadcast"),
            2,
        );

        drop(tcp);
        assert_eq!(hub.online_count(), 1);
        clock.set_minutes(10);
        scheduler.tick();
        assert_online(
            &web.recv()
                .await
                .expect("remaining WebSocket should receive"),
            1,
        );
        assert_line_message(
            &web.recv()
                .await
                .expect("remaining WebSocket should receive line message"),
        );
        drop(web);
        assert_eq!(hub.online_count(), 0);
    }

    #[test]
    fn presence_unregisters_when_connection_guard_is_dropped() {
        let hub = deterministic_hub();
        let mut connection = Some(hub.register(ChatProtocol::Tcp));
        assert_eq!(hub.online_count(), 1);
        connection.take();
        assert_eq!(hub.online_count(), 0);
    }

    #[test]
    fn qa_overrides_fail_closed_without_nonempty_control_token() {
        let mut values = HashMap::from([
            (QA_ONLINE_INTERVAL_MS_ENV, "25".to_string()),
            (QA_LINE_INTERVAL_MS_ENV, "50".to_string()),
            (QA_FIXED_LINE_INDEX_ENV, "2".to_string()),
        ]);
        let config = ChatBroadcastConfig::from_lookup(|name| values.get(name).cloned());
        assert_eq!(config.online_interval, ONLINE_INTERVAL);
        assert_eq!(config.line_message_interval, LINE_MESSAGE_INTERVAL);
        assert_eq!(config.fixed_line_index, None);
        assert_eq!(config.max_packets, None);

        values.insert(QA_CONTROL_TOKEN_ENV, String::new());
        let config = ChatBroadcastConfig::from_lookup(|name| values.get(name).cloned());
        assert_eq!(config.online_interval, ONLINE_INTERVAL);
        assert_eq!(config.line_message_interval, LINE_MESSAGE_INTERVAL);
        assert_eq!(config.fixed_line_index, None);
        assert_eq!(config.max_packets, None);

        values.insert(QA_CONTROL_TOKEN_ENV, "enabled".to_string());
        values.insert(QA_MAX_PACKETS_ENV, "4".to_string());
        let config = ChatBroadcastConfig::from_lookup(|name| values.get(name).cloned());
        assert_eq!(config.online_interval, Duration::from_millis(25));
        assert_eq!(config.line_message_interval, Duration::from_millis(50));
        assert_eq!(config.fixed_line_index, Some(2));
        assert_eq!(config.max_packets, Some(4));
    }

    #[test]
    fn qa_packet_limit_waits_for_presence_and_stops_on_original_order() {
        let hub = ChatBroadcastHub::new(
            ChatBroadcastConfig {
                online_interval: Duration::from_millis(500),
                line_message_interval: Duration::from_millis(750),
                fixed_line_index: Some(1),
                max_packets: Some(4),
            },
            vec!["first line".to_string(), "fixed line".to_string()],
        );
        let clock = Arc::new(ManualClock::default());
        let mut scheduler = hub.scheduler_with_clock(clock.clone(), 13);
        clock.elapsed_ms.store(2_000, Ordering::Release);
        assert!(scheduler.tick().is_empty());

        let _presence = hub.register(ChatProtocol::WebSocket);
        let packets = scheduler.tick();
        assert_eq!(packets.len(), 4);
        assert_online(&packets[0], 1);
        assert_line_message(&packets[1]);
        assert_online(&packets[2], 1);
        assert_online(&packets[3], 1);

        clock.elapsed_ms.store(20_000, Ordering::Release);
        assert!(scheduler.tick().is_empty());
    }
}
