//! Real local named-pipe tests. Never derive names from InstanceIdentity or
//! invoke start_server(): every test owns a separate, unpredictable namespace.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_name() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!(
        r"\\.\pipe\QuickNest-IPC-Isolated-Test-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn server() -> PipeServer {
    start_server_named(unique_name()).expect("isolated secured pipe must start")
}

fn client(name: &str) -> Arc<OwnedHandle> {
    connect_overlapped(name, Instant::now() + CONNECT_BUDGET).unwrap()
}

fn send_request(pipe: &Arc<OwnedHandle>) {
    write_exact(
        pipe,
        &IpcMessage::new(IpcCommand::Show).encode().unwrap(),
        Instant::now() + CONNECT_BUDGET,
        None,
    )
    .unwrap();
}

fn receive_command(server: &PipeServer) {
    assert_eq!(
        server.events.as_ref().unwrap().recv_timeout(CONNECT_BUDGET),
        Ok(IpcCommand::Show)
    );
}

fn fake_server(
    action: impl FnOnce(Arc<OwnedHandle>, Arc<StopEvent>) + Send + 'static,
) -> PipeServer {
    let name = unique_name();
    let pipe = create_server_pipe(&name).unwrap();
    let stop = Arc::new(StopEvent::new().unwrap());
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        accept(&pipe, Instant::now() + CONNECT_BUDGET, &thread_stop).unwrap();
        action(pipe, thread_stop);
    });
    PipeServer {
        name,
        events: None,
        thread: Some(thread),
        stop,
    }
}

fn assert_bounded_drop(server: PipeServer) {
    let thread = server.thread.as_ref().unwrap().as_raw_handle();
    let mut duplicate = std::ptr::null_mut();
    // Keep an independent thread handle so the test verifies actual worker exit,
    // not merely the bounded detach fallback in Drop.
    unsafe {
        assert_ne!(
            windows_sys::Win32::Foundation::DuplicateHandle(
                GetCurrentProcess(),
                thread,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                0,
                windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS,
            ),
            0
        );
    }
    let duplicate = unsafe { OwnedHandle::from_raw_handle(duplicate) };
    let started = Instant::now();
    drop(server);
    assert!(started.elapsed() < SHUTDOWN_BUDGET + Duration::from_millis(250));
    assert_eq!(
        unsafe { WaitForSingleObject(duplicate.as_raw_handle(), 0) },
        WAIT_OBJECT_0
    );
}

#[test]
fn ipc_idle_client_expires_and_next_client_is_served() {
    let server = server();
    let idle = client(server.name());
    // Hold an idle client past its session budget, then use a fresh complete
    // client budget. The old handle deliberately remains open throughout.
    std::thread::sleep(CONNECT_BUDGET + Duration::from_millis(150));
    assert_eq!(
        send_message(server.name(), &IpcMessage::new(IpcCommand::Show)),
        Ok(true)
    );
    receive_command(&server);
    drop(idle);
}

#[test]
fn ipc_ack_survives_delayed_read_and_repeated_connections() {
    let server = server();
    let pipe = client(server.name());
    send_request(&pipe);
    receive_command(&server);
    std::thread::sleep(Duration::from_millis(150));
    let ack = read_message(&pipe, ACK_CAPACITY, Instant::now() + CONNECT_BUDGET, None).unwrap();
    assert_eq!(parse_ack(&ack), Ok(true));
    drop(pipe);
    for _ in 0..12 {
        assert_eq!(
            send_message(server.name(), &IpcMessage::new(IpcCommand::Show)),
            Ok(true)
        );
        receive_command(&server);
    }
}

#[test]
fn ipc_unread_ack_expires_without_flush_or_permanent_occupation() {
    let server = server();
    let stalled = client(server.name());
    send_request(&stalled);
    receive_command(&server);
    std::thread::sleep(CONNECT_BUDGET + Duration::from_millis(150));
    assert_eq!(
        send_message(server.name(), &IpcMessage::new(IpcCommand::Show)),
        Ok(true)
    );
    receive_command(&server);
    drop(stalled);
}

#[test]
fn ipc_drop_cancels_pending_accept_and_joins() {
    let server = server();
    std::thread::sleep(Duration::from_millis(40));
    assert_bounded_drop(server);
}

#[test]
fn ipc_drop_cancels_pending_read_and_joins() {
    let server = server();
    let idle = client(server.name());
    std::thread::sleep(Duration::from_millis(40));
    assert_bounded_drop(server);
    drop(idle);
}

#[test]
fn ipc_drop_during_unread_ack_is_bounded_and_joins() {
    let server = server();
    let pipe = client(server.name());
    send_request(&pipe);
    receive_command(&server);
    assert_bounded_drop(server);
}

#[test]
fn ipc_client_silent_server_uses_total_deadline() {
    let server = fake_server(|pipe, stop| {
        read_message(
            &pipe,
            FRAME_CAPACITY,
            Instant::now() + CONNECT_BUDGET,
            Some(&stop),
        )
        .unwrap();
        unsafe { WaitForSingleObject(stop.handle(), 4000) };
    });
    let started = Instant::now();
    assert_eq!(
        send_message(server.name(), &IpcMessage::new(IpcCommand::Show)),
        Err(IpcError::TimedOut)
    );
    assert!(started.elapsed() >= CONNECT_BUDGET - Duration::from_millis(100));
    assert!(started.elapsed() < CONNECT_BUDGET + Duration::from_millis(350));
    assert_bounded_drop(server);
}

#[test]
fn ipc_raw_client_silent_server_uses_total_deadline() {
    let server = fake_server(|_pipe, stop| {
        unsafe { WaitForSingleObject(stop.handle(), 4000) };
    });
    let started = Instant::now();
    assert_eq!(
        send_raw(server.name(), &[0, 0, 0, 0]),
        Err(IpcError::TimedOut)
    );
    assert!(started.elapsed() < CONNECT_BUDGET + Duration::from_millis(350));
}

#[test]
fn ipc_connect_and_ack_share_deadline_after_busy_instance() {
    let name = unique_name();
    let occupied = create_server_pipe(&name).unwrap();
    let blocker = client(&name);
    let stop = Arc::new(StopEvent::new().unwrap());
    accept(&occupied, Instant::now() + CONNECT_BUDGET, &stop).unwrap();
    let worker_stop = Arc::clone(&stop);
    let worker_pipe = Arc::clone(&occupied);
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        unsafe { DisconnectNamedPipe(worker_pipe.as_raw_handle()) };
        accept(&worker_pipe, Instant::now() + CONNECT_BUDGET, &worker_stop).unwrap();
        read_message(
            &worker_pipe,
            FRAME_CAPACITY,
            Instant::now() + CONNECT_BUDGET,
            Some(&worker_stop),
        )
        .unwrap();
        unsafe { WaitForSingleObject(worker_stop.handle(), 4000) };
    });
    let started = Instant::now();
    assert_eq!(
        send_message(&name, &IpcMessage::new(IpcCommand::Show)),
        Err(IpcError::TimedOut)
    );
    assert!(
        started.elapsed() < CONNECT_BUDGET + Duration::from_millis(350),
        "connection time must not reset ACK budget"
    );
    stop.signal();
    worker.join().unwrap();
    drop(blocker);
}

#[test]
fn ipc_missing_pipe_and_busy_pipe_honor_connection_deadline() {
    let started = Instant::now();
    assert_eq!(connect_client(&unique_name()), Err(IpcError::TimedOut));
    assert!(started.elapsed() < CONNECT_BUDGET + Duration::from_millis(350));
    let name = unique_name();
    let _pipe = create_server_pipe(&name).unwrap();
    let _occupied = client(&name);
    let started = Instant::now();
    assert_eq!(connect_client(&name), Err(IpcError::TimedOut));
    assert!(started.elapsed() < CONNECT_BUDGET + Duration::from_millis(350));
}

#[test]
fn ipc_ready_errors_are_returned_before_start_succeeds() {
    let server = server();
    assert!(matches!(
        start_server_named(server.name().to_owned()),
        Err(IpcError::Failed(_))
    ));
    assert!(matches!(
        start_server_named("invalid-pipe-path".into()),
        Err(IpcError::Failed(_))
    ));
}

#[test]
fn ipc_rejected_frames_deliver_ack_without_dispatch() {
    let server = server();
    assert_eq!(send_raw(server.name(), &[0, 0, 0, 0]), Ok(Some(false)));
    let receiver = server.events.as_ref().unwrap();
    assert_eq!(
        receiver.recv_timeout(Duration::from_millis(40)),
        Err(RecvTimeoutError::Timeout)
    );
    assert_eq!(
        send_message(server.name(), &IpcMessage::new(IpcCommand::Show)),
        Ok(true)
    );
    receive_command(&server);
}

#[test]
fn ipc_pending_write_is_cancelled_at_deadline_and_on_stop() {
    // A private large write forces backpressure against a peer which never
    // reads. Public send_raw remains capped at FRAME_CAPACITY.
    for cancel in [false, true] {
        let name = unique_name();
        let pipe = create_server_pipe(&name).unwrap();
        let _peer = client(&name);
        let stop = Arc::new(StopEvent::new().unwrap());
        accept(&pipe, Instant::now() + CONNECT_BUDGET, &stop).unwrap();
        let signaler = if cancel {
            let stop = Arc::clone(&stop);
            Some(std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(40));
                stop.signal();
            }))
        } else {
            None
        };
        let started = Instant::now();
        assert_eq!(
            write_exact(
                &pipe,
                &vec![0; 1024 * 1024],
                started + Duration::from_millis(200),
                Some(&stop)
            ),
            Err(IpcError::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_millis(450));
        if let Some(signaler) = signaler {
            signaler.join().unwrap();
        }
    }
}
