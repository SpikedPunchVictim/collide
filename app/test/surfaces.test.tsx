import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { ComparePane } from '../src/components/compare/ComparePane.tsx'
import { ExperimentsSurface } from '../src/components/experiments/ExperimentsSurface.tsx'
import { FieldSurface } from '../src/components/field/FieldSurface.tsx'
import {
  PLACEHOLDER_CONTEXT,
  PLACEHOLDER_NOW_MS,
  PLACEHOLDER_OUTPUT,
  PLACEHOLDER_QUEUE,
  PLACEHOLDER_RUNS,
  PLACEHOLDER_TASKS,
} from '../src/components/placeholders.ts'
import { QueueSurface } from '../src/components/queue/QueueSurface.tsx'
import { OutputPane } from '../src/components/run/OutputPane.tsx'
import { ReviewPane } from '../src/components/run/ReviewPane.tsx'
import { RunDetail } from '../src/components/run/RunDetail.tsx'
import { TimeBreakdown } from '../src/components/run/TimeBreakdown.tsx'
import { TracksSurface } from '../src/components/tracks/TracksSurface.tsx'
import {
  PLACEHOLDER_BREAKDOWN,
  PLACEHOLDER_EXPERIMENT,
  PLACEHOLDER_REVIEW,
} from '../src/components/placeholders-detail.ts'

// One smoke test for each surface: it renders with placeholder props, with no browser (`renderToString`).

const noop = () => {}

describe('Field', () => {
  const props = { selected: [], lens: null, highlight: null, onToggleTask: noop, onLaunch: noop, onClearLens: noop }

  test('renders its tasks in plain words', () => {
    const html = renderToString(<FieldSurface tasks={PLACEHOLDER_TASKS} {...props} />)
    expect(html).toContain('Field')
    expect(html).toContain('Remember the task list between launches')
    expect(html).toContain('Ready to start')
  })

  test('an empty board still offers a way in', () => {
    const html = renderToString(<FieldSurface tasks={[]} {...props} />)
    expect(html).toContain('New session')
    expect(html).toContain('Add a pack')
  })

  test('names the number of selected tasks on the Launch button', () => {
    const html = renderToString(<FieldSurface tasks={PLACEHOLDER_TASKS} {...props} selected={['T-03-011']} />)
    expect(html).toContain('Launch 1 task')
  })
})

describe('Tracks', () => {
  test('renders every run by its label, with its state in words', () => {
    const html = renderToString(
      <TracksSurface
        runs={PLACEHOLDER_RUNS}
        lens={null}
        highlight={null}
        onOpenRun={noop}
        onCompare={noop}
        onClearLens={noop}
        compare={null}
      />,
    )
    expect(html).toContain('R-118')
    expect(html).toContain('Working')
    expect(html).toContain('Waiting for your answer')
  })
})

describe('Queue', () => {
  test('renders the waiting decisions and the detail of the first one', () => {
    const html = renderToString(
      <QueueSurface
        items={PLACEHOLDER_QUEUE}
        selected={null}
        context={PLACEHOLDER_CONTEXT}
        nowMs={PLACEHOLDER_NOW_MS}
        onSelect={noop}
        onAnswer={noop}
      />,
    )
    expect(html).toContain('Queue')
    expect(html).toContain('The agent wants to run a command')
    expect(html).toContain('14 min')
    expect(html).toContain('Why is this asked?')
  })

  test('flags a command that came from untrusted text, and shows a missing input as such', () => {
    const flagged = renderToString(
      <QueueSurface items={PLACEHOLDER_QUEUE} selected="Q-1" context={null} nowMs={PLACEHOLDER_NOW_MS} onSelect={noop} onAnswer={noop} />,
    )
    expect(flagged).toContain('pnpm install')
    expect(flagged).toContain('untrusted')

    const [first] = PLACEHOLDER_QUEUE
    const withoutInput = { ...first!, permission: { ...first!.permission!, input: undefined } }
    const html = renderToString(
      <QueueSurface items={[withoutInput]} selected="Q-1" context={null} nowMs={PLACEHOLDER_NOW_MS} onSelect={noop} onAnswer={noop} />,
    )
    expect(html).toContain('input not available until approved')
  })

  test('an empty Queue says so', () => {
    const html = renderToString(
      <QueueSurface items={[]} selected={null} context={null} nowMs={PLACEHOLDER_NOW_MS} onSelect={noop} onAnswer={noop} />,
    )
    expect(html).toContain('Nothing is waiting on you')
  })
})

describe('Run detail', () => {
  const run = PLACEHOLDER_RUNS[0]!
  const detail = (tab: 'output' | 'time' | 'review') =>
    renderToString(
      <RunDetail
        run={run}
        entries={PLACEHOLDER_OUTPUT}
        breakdown={PLACEHOLDER_BREAKDOWN}
        review={PLACEHOLDER_REVIEW}
        tab={tab}
        onDecide={noop}
      />,
    )

  test('shows the run and its three tabs', () => {
    const html = detail('output')
    expect(html).toContain('R-118')
    for (const name of ['Output', 'Time breakdown', 'Review']) expect(html).toContain(name)
  })

  test('the Output tab renders the agent output with Jump to latest', () => {
    expect(detail('output')).toContain('Jump to latest')
  })

  test('the Time breakdown tab renders the table', () => {
    expect(detail('time')).toContain('Time spent on')
  })

  test('the Review tab renders both reviewers', () => {
    const html = detail('review')
    expect(html).toContain('Reviewer A')
    expect(html).toContain('Reviewer B')
    expect(html).toContain('Accept')
  })

  test('each pane renders on its own', () => {
    expect(renderToString(<OutputPane entries={PLACEHOLDER_OUTPUT} />)).toContain('I will start by reading the failing test.')
    expect(renderToString(<TimeBreakdown rows={PLACEHOLDER_BREAKDOWN} />)).toContain('unattributed')
    expect(renderToString(<ReviewPane run={run} review={PLACEHOLDER_REVIEW} onDecide={noop} />)).toContain('Discard')
  })

  test('agent text is shown as text, never as markup', () => {
    const hostile = [{ index: 0, wall_ms: 0, event: { type: 'message' as const, text: '<img src=x onerror=alert(1)>' } }]
    const html = renderToString(<OutputPane entries={hostile} />)
    expect(html).not.toContain('<img')
    expect(html).toContain('&lt;img')
  })
})

describe('Compare', () => {
  test('renders two runs side by side with no verdict', () => {
    const html = renderToString(<ComparePane left={PLACEHOLDER_RUNS[2]!} right={PLACEHOLDER_RUNS[3]!} />)
    expect(html).toContain('Compare runs')
    expect(html).toContain('R-115')
    expect(html).toContain('R-116')
    expect(html).toContain('fix')
    expect(html).not.toContain('Verdict')
  })
})

describe('Experiments', () => {
  test('renders the two arms, the preview and the scope of a result', () => {
    const html = renderToString(<ExperimentsSurface experiment={PLACEHOLDER_EXPERIMENT} onStart={noop} />)
    expect(html).toContain('Experiments')
    expect(html).toContain('Arm A')
    expect(html).toContain('Arm B')
    expect(html).toContain('on this task')
  })
})
