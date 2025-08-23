import { render } from 'solid-js/web'
import App from './App'

const root = document.getElementById('root')

if (import.meta.hot) {
  import.meta.hot.dispose(() => root.textContent = '')
}

render(() => <App />, root)
