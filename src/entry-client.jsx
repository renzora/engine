import { render } from 'solid-js/web'
import App from './App'

// Initialize renderers early to ensure registration happens
import '@/api/render'

render(() => <App />, document.getElementById('root'))
