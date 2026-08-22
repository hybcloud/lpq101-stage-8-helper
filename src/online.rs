use std::{
    io::ErrorKind,
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use serde::Deserialize;
use tungstenite::{
    Message, WebSocket, client::IntoClientRequest, connect, http::HeaderValue,
    stream::MaybeTlsStream,
};

const OWNER_COOKIE_NAME: &str = "lpq_stage8_owner";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const SOCKET_READ_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineRole {
    Host,
    Viewer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineAction {
    Next,
    Previous,
    Reset,
    Sync(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteMovement {
    pub from_box: u8,
    pub to_box: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OnlineEvent {
    Connected {
        role: OnlineRole,
        code: String,
        invite_url: Option<String>,
    },
    State {
        index: usize,
        movement: Option<RemoteMovement>,
        instruction: String,
    },
    Error(String),
}

enum OnlineCommand {
    Action(OnlineAction),
    Stop,
}

#[derive(Deserialize)]
struct CreateRoomResponse {
    state: RemoteRoomCode,
}

#[derive(Deserialize)]
struct RemoteRoomCode {
    code: String,
}

#[derive(Deserialize)]
struct SocketEnvelope {
    #[serde(rename = "type")]
    kind: String,
    state: RemoteRoomState,
}

#[derive(Deserialize)]
struct RemoteRoomState {
    index: usize,
    movement: Option<RemoteMovementBody>,
    instruction: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMovementBody {
    from_box: u8,
    to_box: u8,
}

pub struct OnlineClient {
    commands: Option<Sender<OnlineCommand>>,
    events: Receiver<OnlineEvent>,
}

impl OnlineClient {
    pub fn new() -> Self {
        let (_, events) = mpsc::channel();
        Self {
            commands: None,
            events,
        }
    }

    pub fn host(&mut self, service_url: String, owner_guid: String, state_index: usize) {
        self.start(move |commands, events| {
            run_host(&service_url, &owner_guid, state_index, commands, events)
        });
    }

    pub fn view(&mut self, service_url: String, room_code: String) {
        self.start(move |commands, events| run_viewer(&service_url, &room_code, commands, events));
    }

    fn start(
        &mut self,
        run: impl FnOnce(&Receiver<OnlineCommand>, &Sender<OnlineEvent>) -> Result<()> + Send + 'static,
    ) {
        self.disconnect();
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        self.commands = Some(command_tx);
        self.events = event_rx;
        thread::spawn(move || {
            if let Err(error) = run(&command_rx, &event_tx) {
                let _ = event_tx.send(OnlineEvent::Error(format!(
                    "Online sync unavailable · {error}"
                )));
            }
        });
    }

    pub fn send(&self, action: OnlineAction) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(OnlineCommand::Action(action));
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(OnlineCommand::Stop);
        }
        let (_, events) = mpsc::channel();
        self.events = events;
    }

    pub fn poll(&self) -> impl Iterator<Item = OnlineEvent> + '_ {
        self.events.try_iter()
    }
}

impl Drop for OnlineClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn run_host(
    service_url: &str,
    owner_guid: &str,
    state_index: usize,
    commands: &Receiver<OnlineCommand>,
    events: &Sender<OnlineEvent>,
) -> Result<()> {
    let service_url = service_url.trim_end_matches('/');
    let cookie = format!("{OWNER_COOKIE_NAME}={owner_guid}");
    let agent = http_agent();
    let room: CreateRoomResponse = agent
        .post(&format!("{service_url}/api/rooms"))
        .set("Cookie", &cookie)
        .call()
        .context("create or restore room")?
        .into_json()
        .context("read room response")?;
    let code = room.state.code;
    let mut socket = connect_socket(service_url, &code, Some(&cookie))?;
    post_action(
        &agent,
        service_url,
        &code,
        &cookie,
        OnlineAction::Sync(state_index),
    )?;
    events
        .send(OnlineEvent::Connected {
            role: OnlineRole::Host,
            code: code.clone(),
            invite_url: Some(format!("{service_url}/room/{code}")),
        })
        .ok();
    run_socket(
        &mut socket,
        OnlineRole::Host,
        commands,
        events,
        Some((&agent, service_url, &code, &cookie)),
    )
}

fn run_viewer(
    service_url: &str,
    room_code: &str,
    commands: &Receiver<OnlineCommand>,
    events: &Sender<OnlineEvent>,
) -> Result<()> {
    let service_url = service_url.trim_end_matches('/');
    let code = room_code.trim().to_ascii_uppercase();
    anyhow::ensure!(
        code.len() == 4
            && code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()),
        "room code must contain four letters or digits"
    );
    let mut socket = connect_socket(service_url, &code, None)?;
    events
        .send(OnlineEvent::Connected {
            role: OnlineRole::Viewer,
            code,
            invite_url: None,
        })
        .ok();
    run_socket(&mut socket, OnlineRole::Viewer, commands, events, None)
}

fn run_socket(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    role: OnlineRole,
    commands: &Receiver<OnlineCommand>,
    events: &Sender<OnlineEvent>,
    owner: Option<(&ureq::Agent, &str, &str, &str)>,
) -> Result<()> {
    set_read_timeout(socket, SOCKET_READ_INTERVAL)?;
    let mut last_heartbeat = Instant::now();
    loop {
        loop {
            match commands.try_recv() {
                Ok(OnlineCommand::Action(action)) if role == OnlineRole::Host => {
                    let (agent, service_url, code, cookie) = owner.expect("host owner context");
                    post_action(agent, service_url, code, cookie, action)?;
                }
                Ok(OnlineCommand::Action(_)) => {}
                Ok(OnlineCommand::Stop) | Err(TryRecvError::Disconnected) => {
                    let _ = socket.close(None);
                    return Ok(());
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        match socket.read() {
            Ok(Message::Text(text)) if role == OnlineRole::Viewer => {
                if let Ok(payload) = serde_json::from_str::<SocketEnvelope>(text.as_ref())
                    && payload.kind == "state"
                {
                    events
                        .send(OnlineEvent::State {
                            index: payload.state.index,
                            movement: payload.state.movement.map(|movement| RemoteMovement {
                                from_box: movement.from_box,
                                to_box: movement.to_box,
                            }),
                            instruction: payload.state.instruction,
                        })
                        .ok();
                }
            }
            Ok(Message::Close(_)) => anyhow::bail!("room socket closed"),
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                anyhow::bail!("room socket closed")
            }
            Err(error) => return Err(error).context("read room socket"),
        }

        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            socket
                .send(Message::Ping(Vec::new().into()))
                .context("keep room socket alive")?;
            last_heartbeat = Instant::now();
        }
    }
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(8))
        .timeout_write(Duration::from_secs(8))
        .build()
}

fn post_action(
    agent: &ureq::Agent,
    service_url: &str,
    code: &str,
    cookie: &str,
    action: OnlineAction,
) -> Result<()> {
    let body = match action {
        OnlineAction::Next => serde_json::json!({ "action": "next" }),
        OnlineAction::Previous => serde_json::json!({ "action": "previous" }),
        OnlineAction::Reset => serde_json::json!({ "action": "reset" }),
        OnlineAction::Sync(index) => serde_json::json!({ "action": "sync", "index": index }),
    };
    agent
        .post(&format!("{service_url}/api/rooms/{code}/action"))
        .set("Cookie", cookie)
        .send_json(body)
        .context("publish room state")?;
    Ok(())
}

fn connect_socket(
    service_url: &str,
    code: &str,
    cookie: Option<&str>,
) -> Result<WebSocket<MaybeTlsStream<TcpStream>>> {
    let mut request = websocket_url(service_url, code)?
        .into_client_request()
        .context("prepare room socket")?;
    if let Some(cookie) = cookie {
        request.headers_mut().insert(
            "Cookie",
            HeaderValue::from_str(cookie).context("prepare owner cookie")?,
        );
    }
    let (socket, _) = connect(request).context("connect room socket")?;
    Ok(socket)
}

fn set_read_timeout(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) -> Result<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(Some(timeout))?,
        MaybeTlsStream::NativeTls(stream) => stream.get_mut().set_read_timeout(Some(timeout))?,
        _ => {}
    }
    Ok(())
}

fn websocket_url(service_url: &str, code: &str) -> Result<String> {
    if let Some(rest) = service_url.strip_prefix("https://") {
        return Ok(format!("wss://{rest}/api/rooms/{code}/socket"));
    }
    if let Some(rest) = service_url.strip_prefix("http://") {
        return Ok(format!("ws://{rest}/api/rooms/{code}/socket"));
    }
    anyhow::bail!("room service URL must start with http:// or https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_event(client: &OnlineClient) -> OnlineEvent {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(event) = client.poll().next() {
                return event;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for online event"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn websocket_urls_follow_the_room_service_scheme() {
        assert_eq!(
            websocket_url("https://example.com", "A1B2").unwrap(),
            "wss://example.com/api/rooms/A1B2/socket"
        );
        assert_eq!(
            websocket_url("http://127.0.0.1:8787", "9XYZ").unwrap(),
            "ws://127.0.0.1:8787/api/rooms/9XYZ/socket"
        );
        assert!(websocket_url("example.com", "A1B2").is_err());
    }

    #[test]
    #[ignore = "requires the local Worker dev server on 127.0.0.1:8787"]
    fn local_host_and_viewer_round_trip() {
        let service_url = "http://127.0.0.1:8787".to_owned();
        let mut host = OnlineClient::new();
        host.host(service_url.clone(), uuid::Uuid::new_v4().to_string(), 0);
        let code = match next_event(&host) {
            OnlineEvent::Connected {
                role: OnlineRole::Host,
                code,
                ..
            } => code,
            event => panic!("unexpected host event: {event:?}"),
        };

        let mut viewer = OnlineClient::new();
        viewer.view(service_url, code);
        assert!(matches!(
            next_event(&viewer),
            OnlineEvent::Connected {
                role: OnlineRole::Viewer,
                ..
            }
        ));
        assert!(matches!(
            next_event(&viewer),
            OnlineEvent::State { index: 0, .. }
        ));

        host.send(OnlineAction::Next);
        assert!(matches!(
            next_event(&viewer),
            OnlineEvent::State { index: 1, .. }
        ));
        host.disconnect();
        viewer.disconnect();
    }
}
