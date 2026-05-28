use bevy::{prelude::Deref, reflect::Reflect};

#[derive(Default, Debug, Clone, Reflect)]
#[reflect]
pub struct SlotMap<T> {
    data: Vec<Option<T>>,
    unused: Vec<SlotId>,
}

#[derive(Deref, Debug, Copy, Clone, Reflect)]
#[reflect]
pub struct SlotId(usize);

impl Default for SlotId {
    fn default() -> Self {
        Self(usize::MAX)
    }
}

impl<T> SlotMap<T> {
    pub fn insert(&mut self, value: T) -> SlotId {
        match self.unused.pop() {
            Some(id) => {
                self.data[*id] = Some(value);
                id
            }
            None => {
                self.data.push(Some(value));
                SlotId(self.data.len() - 1)
            }
        }
    }

    pub fn get(&self, id: SlotId) -> Option<&T> {
        self.data.get(*id).map(|v| v.as_ref()).flatten()
    }

    pub fn get_mut(&mut self, id: SlotId) -> Option<&mut T> {
        self.data.get_mut(*id).map(|v| v.as_mut()).flatten()
    }

    pub fn remove(&mut self, id: SlotId) -> Option<T> {
        match self.data.remove(*id) {
            Some(val) => {
                self.unused.push(id);
                Some(val)
            }
            None => None,
        }
    }
}
