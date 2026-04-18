//! Modal: backdrop click-to-close and close button behavior.
//!
//! Structure:
//! - Root entity: ModalData
//!   - Child "backdrop": full-screen overlay, click to close
//!   - Child "content": the modal content panel
//!   - Child "close_button": optional close button

use bevy::prelude::*;

use crate::components::{ModalData, UiWidgetPart};

pub fn modal_system(
    mut modals: Query<(Entity, &ModalData, &Children, &mut Visibility)>,
    parts: Query<(&UiWidgetPart, &Interaction), Changed<Interaction>>,
) {
    for (_entity, data, children, mut vis) in &mut modals {
        for child in children.iter() {
            let Ok((part, interaction)) = parts.get(child) else {
                continue;
            };

            if *interaction != Interaction::Pressed {
                continue;
            }

            match part.role.as_str() {
                "backdrop" if data.closable => {
                    *vis = Visibility::Hidden;
                }
                "close_button" if data.closable => {
                    *vis = Visibility::Hidden;
                }
                _ => {}
            }
        }
    }
}
