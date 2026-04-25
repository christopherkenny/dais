use crate::commands::Command;

/// MPSC command bus for dispatching user actions to the presentation engine.
///
/// All user actions — from keyboard input, mouse events, or future external
/// control surfaces — are sent as [`Command`] values through this bus.
/// The engine polls the receiving end each frame via `try_recv()`.
pub struct CommandBus {
    sender: crossbeam_channel::Sender<Command>,
    receiver: crossbeam_channel::Receiver<Command>,
}

/// Cloneable handle for sending commands into the bus.
///
/// Input sources (keyboard handler, mouse handler, future REST API, etc.)
/// each hold a `CommandSender` and dispatch commands independently.
#[derive(Clone)]
pub struct CommandSender {
    inner: crossbeam_channel::Sender<Command>,
}

/// Receiving end of the command bus, held by the engine.
pub struct CommandReceiver {
    inner: crossbeam_channel::Receiver<Command>,
}

impl CommandBus {
    /// Create a new command bus.
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }

    /// Get a cloneable sender handle for dispatching commands.
    pub fn sender(&self) -> CommandSender {
        CommandSender { inner: self.sender.clone() }
    }

    /// Consume the bus and return the receiver (for the engine).
    pub fn into_receiver(self) -> CommandReceiver {
        CommandReceiver { inner: self.receiver }
    }
}

impl Default for CommandBus {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSender {
    /// Send a command to the engine. Returns an error if the receiver is dropped.
    pub fn send(&self, command: Command) -> Result<(), crossbeam_channel::SendError<Command>> {
        self.inner.send(command)
    }
}

impl CommandReceiver {
    /// Try to receive a command without blocking. Returns `None` if the bus is empty.
    pub fn try_recv(&self) -> Option<Command> {
        self.inner.try_recv().ok()
    }

    /// Drain all pending commands from the bus.
    pub fn drain(&self) -> Vec<Command> {
        let mut commands = Vec::new();
        while let Some(cmd) = self.try_recv() {
            commands.push(cmd);
        }
        commands
    }
}
