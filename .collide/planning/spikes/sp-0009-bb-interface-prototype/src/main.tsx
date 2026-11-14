import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { reduce } from './reducer'
import { scenarioById } from './scenarios'
import { agentDemoState } from './agentDemo'
import { initialState } from './world'
import { App } from './ui/App'
import { Provider } from './ui/store'
import './styles.css'

const params = new URLSearchParams(location.search)
const facilitator = params.has('facilitator')
// ?scenario=4 preloads a scenario (facilitator convenience); the timer still waits for Start
const sc = params.get('scenario')
// without a scenario the page starts in agent-demo mode: the Larder tasks and a few finished runs (src/agentDemo.ts)
const initial = sc && scenarioById(sc)
  ? reduce(initialState('orchard', facilitator), { type: 'loadScenario', id: sc, t: Date.now() })
  : agentDemoState(facilitator)

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Provider facilitator={facilitator} initial={initial}>
      <App />
    </Provider>
  </StrictMode>,
)
