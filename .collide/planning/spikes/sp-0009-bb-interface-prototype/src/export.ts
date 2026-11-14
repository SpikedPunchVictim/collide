import type { State } from './types'

/** The session log the facilitator saves: one entry per attempt, with variants, timing, errors, notes and the action log. */
export function exportSession(s: State, now = new Date()): object {
  const ss = s.session
  return {
    tool: 'collide-s6-prototype',
    version: 1,
    exportedAt: now.toISOString(),
    participant: ss.participant,
    attempts: ss.attempts.map((a) => ({
      scenario: a.scenario,
      title: a.title,
      variants: a.variants,
      startedAt: new Date(a.startedAt).toISOString(),
      seconds: Math.round(a.ms / 100) / 10,
      errors: a.errors,
      errorLog: a.errorLog.map((e) => ({ atSeconds: Math.round((e.t - a.startedAt) / 100) / 10, note: e.note, auto: e.auto })),
      autoFlags: a.autoFlags,
      stop: a.stop,
      detected: a.detected,
      correct: a.correct,
      notes: a.notes,
      finding: a.finding,
      decisions: a.decisions.map((d) => ({ ...d, atSeconds: Math.round((d.t - a.startedAt) / 100) / 10 })),
      missing: a.missing,
      actions: a.log.map((l) => ({ atSeconds: Math.round((l.t - a.startedAt) / 100) / 10, action: l.a, detail: l.d })),
    })),
  }
}
