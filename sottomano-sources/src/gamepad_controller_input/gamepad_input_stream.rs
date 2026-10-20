use std::time::Instant;

use futures::{Stream, StreamExt, stream, stream::BoxStream};
use tokio::time::timeout_at;

use super::{
    evdev_gamepad_devices::{RawGamepadEvent, raw_gamepad_events},
    gamepad_input_machine::{
        GamepadInput, GamepadMachine, GamepadStreamSettings, MachineEffect, MachineInput,
    },
    wlr_virtual_pointer_motion::VirtualPointer,
};

struct GamepadDriver {
    machine: GamepadMachine,
    raw: BoxStream<'static, RawGamepadEvent>,
    pointer: Option<VirtualPointer>,
}

impl GamepadDriver {
    async fn next_input(&mut self) -> Option<MachineInput> {
        let Some(deadline) = self.machine.next_deadline() else {
            return self.raw.next().await.map(MachineInput::Raw);
        };
        match timeout_at(deadline.into(), self.raw.next()).await {
            Ok(raw) => raw.map(MachineInput::Raw),
            Err(_) => Some(MachineInput::Tick),
        }
    }

    async fn apply(&mut self, effects: Vec<MachineEffect>) -> Vec<GamepadInput> {
        let mut delivered = Vec::new();
        for effect in effects {
            match (effect, self.pointer.as_mut()) {
                (MachineEffect::Deliver(input), _) => delivered.push(input),
                (MachineEffect::Pointer(effect), Some(pointer)) => {
                    if let Err(e) = pointer.apply(effect).await {
                        tracing::warn!("gamepad: pointer steering stopped: {e}");
                        self.pointer = None;
                    }
                }
                (MachineEffect::Pointer(_), None) => {}
            }
        }
        delivered
    }
}

async fn connected_pointer(settings: &GamepadStreamSettings) -> Option<VirtualPointer> {
    if !settings.steering.needs_virtual_pointer() {
        return None;
    }
    VirtualPointer::connect()
        .await
        .inspect_err(|e| tracing::warn!("gamepad: pointer steering unavailable: {e}"))
        .ok()
}

fn driven_inputs(driver: GamepadDriver) -> impl Stream<Item = GamepadInput> + Send {
    stream::unfold(driver, |mut driver| async move {
        let input = driver.next_input().await?;
        let effects = driver.machine.step(input, Instant::now());
        let delivered = driver.apply(effects).await;
        Some((stream::iter(delivered), driver))
    })
    .flatten()
}

pub fn gamepad_inputs(settings: GamepadStreamSettings) -> impl Stream<Item = GamepadInput> + Send {
    stream::once(async move {
        let pointer = connected_pointer(&settings).await;
        GamepadDriver {
            machine: GamepadMachine::new(settings),
            raw: raw_gamepad_events().boxed(),
            pointer,
        }
    })
    .flat_map(driven_inputs)
}
