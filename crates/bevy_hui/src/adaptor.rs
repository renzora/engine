use bevy::asset::{Asset, AssetPath, AssetServer, Handle, LoadContext};

pub trait AssetLoadAdaptor {
    fn load<'a, A: Asset>(&mut self, path: impl Into<AssetPath<'a>>) -> Handle<A>;
}


pub struct AssetServerAdaptor<'a> {
    pub server: &'a AssetServer,
}

impl<'a> AssetLoadAdaptor for AssetServerAdaptor<'a> {
    fn load<'b, A: Asset>(&mut self, path: impl Into<AssetPath<'b>>) -> Handle<A> {
        self.server.load(path)
    }
}

pub struct LoadContextAdaptor<'a, 'b> {
    pub load_context: &'a mut LoadContext<'b>,
}

impl<'a, 'b> AssetLoadAdaptor for LoadContextAdaptor<'a, 'b> {
    fn load<'c, A: Asset>(&mut self, path: impl Into<AssetPath<'c>>) -> Handle<A> {
        self.load_context.load(path)
    }
}
