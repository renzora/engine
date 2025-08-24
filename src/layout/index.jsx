import TopMenu from './topMenu.jsx';
import Toolbar from './Toolbar.jsx';
import Viewport from './viewport.jsx';
import RightPanel from './rightPanel.jsx';
import BottomPanel from './bottomPanel.jsx';
import Footer from './Footer.jsx';

const Layout = () => {
  return (
    <div class="fixed inset-0 flex flex-col pointer-events-none z-10" onContextMenu={(e) => e.preventDefault()}>
      <div class="flex-shrink-0 pointer-events-auto z-50">
        <TopMenu />
        <Toolbar />
      </div>
    
      <div class="flex-1 relative overflow-hidden pointer-events-auto">
        <Viewport />
        <RightPanel />
        <BottomPanel />
      </div>
      
      <Footer />
    </div>
  );
};

export default Layout;