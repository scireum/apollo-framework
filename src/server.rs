//! Contains the server component of Apollo.
//!
//! Opens a server-socket on the specified port (**server.port** in the config or 2410 as fallback)
//! and binds it to the selected IP (**server.host** in the config or 0.0.0.0 as fallback). Each
//! incoming client is expected to send RESP requests and will be provided with the appropriate
//! responses.
//!
//! Note that in order to achieve zero downtime / ultra high availability demands, the sever will
//! periodically try to bind the socket to the selected port, therefore an "new" instance can
//! be started and the "old" once can bleed out and the port will be "handed through" with minimal
//! downtime. Also, this will listen to change events of the config and will relocate to another
//! port or host if changed.
//!
//! # Example
//!
//! ```no_run
//! use apollo_framework::server::Server;
//! use tokio::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//! }
//! ```
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};

use crate::config::Config;
use crate::platform::Platform;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

/// Specifies the timeout when waiting for a new incoming connection.
///
/// When waiting for a new connection we need to interrupt this every once in a while so that
/// we can check if the platform has been shut down.
const CONNECT_WAIT_TIMEOUT: Duration = Duration::from_millis(500);

/// Represents a client connection.
pub struct Connection<P: Default + Send + Sync> {
    peer_address: String,
    active: AtomicBool,
    payload: P,
}

impl<P: Default + Send + Sync> PartialEq for Connection<P> {
    fn eq(&self, other: &Self) -> bool {
        self.peer_address == other.peer_address
    }
}

impl<P: Default + Send + Sync> Connection<P> {
    /// Determines if the connection is active or if a termination has been requested.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Terminates the connection.
    pub fn quit(&self) {
        self.active.store(false, Ordering::Release);
    }

    /// TODO
    pub fn payload(&self) -> &P {
        &self.payload
    }
}

/// Provides some metadata for a client connection.
pub struct ConnectionInfo<P: Default + Send + Sync> {
    /// Contains the peer address of the client being connected.
    pub peer_address: String,

    /// Contains the name of the connected client.
    pub payload: P,
}

/// Represents a TODO server which manages all TCP connections.
pub struct Server<P: Default + Send + Sync> {
    running: AtomicBool,
    current_address: Mutex<Option<String>>,
    platform: Arc<Platform>,
    connections: Mutex<Vec<Arc<Connection<P>>>>,
}

impl<P: 'static + Default + Send + Sync + Clone> Server<P> {
    /// Creates and installs a **Server** into the given **Platform**.
    ///
    /// Note that this is called by the [Builder](crate::builder::Builder) unless disabled.
    ///
    /// Also note, that this will not technically start the server. This has to be done manually
    /// via [event_loop](Server::event_loop) as it is most probable done in the main thread.
    pub fn install(platform: &Arc<Platform>) -> Arc<Self> {
        let server = Arc::new(Server {
            running: AtomicBool::new(false),
            current_address: Mutex::new(None),
            platform: platform.clone(),
            connections: Mutex::new(Vec::new()),
        });

        platform.register::<Server<P>>(server.clone());

        server
    }

    /// Lists all currently active connections.
    pub fn connections(&self) -> Vec<ConnectionInfo<P>> {
        let mut result = Vec::new();
        for connection in self.connections.lock().unwrap().iter() {
            result.push(ConnectionInfo {
                peer_address: connection.peer_address.clone(),
                payload: connection.payload.clone(),
            });
        }

        result
    }

    /// Kills the connection of the given peer address.
    pub fn kill(&self, peer_address: &str) -> bool {
        self.connections
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.peer_address == peer_address)
            .map(|c| c.active.store(false, Ordering::Release))
            .is_some()
    }

    /// Adds a newly created client connection.
    ///
    /// Note that this involves locking a **Mutex**. However, we expect our clients to use
    /// connection pooling, so that only a few rather long running connections are present.
    fn add_connection(&self, connection: Arc<Connection<P>>) {
        self.connections.lock().unwrap().push(connection);
    }

    /// Removes a connection after it has been closed by either side.
    fn remove_connection(&self, connection: Arc<Connection<P>>) {
        let mut mut_connections = self.connections.lock().unwrap();
        if let Some(index) = mut_connections
            .iter()
            .position(|other| *other == connection)
        {
            let _ = mut_connections.remove(index);
        }
    }

    /// Determines if the server socket should keep listening for incoming connections.
    ///
    /// In contrast to **Platform::is_running** this is not used to control the shutdown of the
    /// server. Rather we toggle this flag to false if a config and therefore address change was
    /// detected. This way **server_loop** will exit and a new server socket for the appropriate
    /// address will be setup by the **event_loop**.
    fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Determines the server address based on the current configuration.
    ///
    /// If no, an invalid or a partial config is present, fallback values are used. By default we
    /// use port 2410 and bind to "0.0.0.0".
    fn address(&self) -> String {
        self.platform
            .find::<Config>()
            .map(|config| {
                let handle = config.current();
                format!(
                    "{}:{}",
                    handle.config()["server"]["host"]
                        .as_str()
                        .unwrap_or("0.0.0.0"),
                    handle.config()["server"]["port"]
                        .as_i64()
                        .filter(|port| port > &0 && port <= &(u16::MAX as i64))
                        .unwrap_or(2410)
                )
            })
            .unwrap_or_else(|| "0.0.0.0:2410".to_owned())
    }

    /// Starts the event loop in a separate thread.
    ///
    /// This is most probably used by test scenarios where the tests itself run in the main thread.
    pub fn fork<F>(
        server: &Arc<Server<P>>,
        client_loop: &'static (impl Fn(Arc<Platform>, Arc<Connection<P>>, TcpStream) -> F + Send + Sync),
    ) where
        F: Future<Output = anyhow::Result<()>> + Send + Sync,
    {
        let cloned_server = server.clone();
        let _ = tokio::spawn(async move {
            cloned_server.event_loop(client_loop).await;
        });
    }

    /// Starts the event loop in a separate thread and waits until the server is up and running.
    ///
    /// Just like **fork** this is intended to be used in test environments.
    pub async fn fork_and_await<F>(
        server: &Arc<Server<P>>,
        client_loop: &'static (impl Fn(Arc<Platform>, Arc<Connection<P>>, TcpStream) -> F + Send + Sync),
    ) where
        F: Future<Output = anyhow::Result<()>> + Send + Sync,
    {
        Server::fork(server, client_loop);

        while !server.is_running() {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// Tries to open a server socket on the specified address to serve incoming client connections.
    ///
    /// The task of this loop is to bind the server socket to the specified address. Once this was
    /// successful, we enter the [server_loop](Server::server_loop) to actually handle incoming
    /// connections. Once this loop returns, either the platform is no longer running and we should
    /// exit, or the config has changed and we should try to bind the server to the new address.
    pub async fn event_loop<F>(
        &self,
        client_loop: impl Fn(Arc<Platform>, Arc<Connection<P>>, TcpStream) -> F
            + Send
            + Sync
            + Copy
            + 'static,
    ) where
        F: Future<Output = anyhow::Result<()>> + Send,
    {
        let mut address = String::new();
        let mut last_bind_error_reported = Instant::now();

        while self.platform.is_running() {
            // If the sever is started for the first time or if it has been restarted due to a
            // config change, we need to reload the address...
            if !self.is_running() {
                address = self.address();
                self.running.store(true, Ordering::Release);
            }

            // Bind and hopefully enter the server_loop...
            if let Ok(mut listener) = TcpListener::bind(&address).await {
                log::info!("Opened server socket on {}...", &address);
                *self.current_address.lock().unwrap() = Some(address.clone());
                self.server_loop(&mut listener, client_loop).await;
                log::info!("Closing server socket on {}.", &address);
            } else {
                // If we were unable to bind to the server, we log this every once in a while
                // (every 5s). Otherwise we would jam the log as re retry every 500ms.
                if Instant::now()
                    .duration_since(last_bind_error_reported)
                    .as_secs()
                    > 5
                {
                    log::error!(
                        "Cannot open server address: {}. Retrying every 500ms...",
                        &address
                    );
                    last_bind_error_reported = Instant::now();
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    /// Runs the main server loop which processes incoming connections.
    ///
    /// This also listens on config changes and exits to the event_loop if necessary (server
    /// address changed...).   
    async fn server_loop<F>(
        &self,
        listener: &mut TcpListener,
        client_loop: impl Fn(Arc<Platform>, Arc<Connection<P>>, TcpStream) -> F
            + Copy
            + Send
            + Sync
            + 'static,
    ) where
        F: Future<Output = anyhow::Result<()>> + Send,
    {
        let mut config_changed_flag = self.platform.require::<Config>().notifier();

        while self.platform.is_running() && self.is_running() {
            tokio::select! {
                // We use a timeout here so that the while condition (esp. platform.is_running())
                // is checked every once in a while...
                timeout_stream = tokio::time::timeout(CONNECT_WAIT_TIMEOUT, listener.accept()) => {
                    // We're only interested in a positive result here, as an Err simply indicates
                    // that the timeout was hit - in this case we do nothing as the while condition
                    // is all the needs to be checked...
                    if let Ok(stream) = timeout_stream {
                        // If a stream is present, we treat this as new connection and eventually
                        // start a client_loop for it...
                        if let Ok((stream, _)) = stream {
                            self.handle_new_connection(stream, client_loop);
                        } else {
                            // Otherwise the socket has been closed therefore we exit to the
                            // event_loop which will either complete exit or try to re-create
                            // the socket.
                            return;
                        }
                    }
                }
                _ = config_changed_flag.recv() => {
                    // If the config was changed, we need to check if the address itself changed...
                    let new_address = self.address();
                    if let Some(current_address) = &*self.current_address.lock().unwrap() {
                       if current_address != &new_address {
                           log::info!("Server address has changed. Restarting server socket...");

                           // Force the event_loop to re-evaluate the expected server address...
                           self.running.store(false, Ordering::Release);

                           // Return to event_loop so that the server socket is re-created...
                           return;
                       }
                    }
               }
            }
        }
    }

    /// Handles a new incoming connection.
    ///
    /// This will register the connection in the list of client connections and then fork a
    /// "thread" which mainly simply executes the **client_loop** for this connection.
    fn handle_new_connection<F>(
        &self,
        stream: TcpStream,
        client_loop: impl FnOnce(Arc<Platform>, Arc<Connection<P>>, TcpStream) -> F
            + 'static
            + Send
            + Sync
            + Copy,
    ) where
        F: Future<Output = anyhow::Result<()>> + Send,
    {
        let platform = self.platform.clone();
        let client_loop = client_loop.clone();
        let _ = tokio::spawn(async move {
            // Mark the connection as nodelay, as we already optimize all writes as far as possible.
            let _ = stream.set_nodelay(true);

            // Register the new connection to that the can report it in the maintenance utilities...
            let server = platform.require::<Server<P>>();
            let connection = Arc::new(Connection {
                peer_address: stream
                    .peer_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|_| "<unknown>".to_owned()),
                active: AtomicBool::new(true),
                payload: P::default(),
            });
            log::debug!("Opened connection from {}...", connection.peer_address);
            server.add_connection(connection.clone());

            // Executes the client loop for this connection....
            if let Err(error) = client_loop(platform, connection.clone(), stream).await {
                log::debug!(
                    "An IO error occurred in connection {}: {}",
                    connection.peer_address,
                    error
                );
            }

            // Removes the connection as it has been closed...
            log::debug!("Closing connection to {}...", connection.peer_address);
            server.remove_connection(connection);
        });
    }
}
