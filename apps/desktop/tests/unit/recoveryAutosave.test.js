import assert from 'node:assert/strict'
import test from 'node:test'
import { RecoveryAutosave } from '../../src/lib/utils/recoveryAutosave.js'

test('debounced recovery flushes pending mutations before writing', async () => {
  const events = []
  let resolveMutation
  const mutation = new Promise((resolve) => {
    resolveMutation = resolve
  })
  const autosave = new RecoveryAutosave({
    delay: 1,
    flush: async () => {
      events.push('flush-start')
      await mutation
      events.push('flush-end')
    },
    write: async () => events.push('write'),
  })

  autosave.schedule()
  await new Promise((resolve) => setTimeout(resolve, 10))
  assert.deepEqual(events, ['flush-start'])
  resolveMutation()
  await new Promise((resolve) => setTimeout(resolve, 10))
  assert.deepEqual(events, ['flush-start', 'flush-end', 'write'])
})

test('a newer schedule cancels the older recovery timer', async () => {
  let writes = 0
  const autosave = new RecoveryAutosave({
    delay: 5,
    flush: async () => {},
    write: async () => { writes += 1 },
  })

  autosave.schedule()
  autosave.schedule()
  await new Promise((resolve) => setTimeout(resolve, 20))
  assert.equal(writes, 1)
})

test('overlapping writes are serialized', async () => {
  let activeWrites = 0
  let maxActiveWrites = 0
  let releaseFirst
  let writes = 0
  const autosave = new RecoveryAutosave({
    flush: async () => {},
    write: async () => {
      writes += 1
      activeWrites += 1
      maxActiveWrites = Math.max(maxActiveWrites, activeWrites)
      if (writes === 1) {
        await new Promise((resolve) => { releaseFirst = resolve })
      }
      activeWrites -= 1
    },
  })

  const first = autosave.runNow()
  await new Promise((resolve) => setTimeout(resolve, 0))
  const second = autosave.runNow()
  releaseFirst()
  await Promise.all([first, second])

  assert.equal(writes, 2)
  assert.equal(maxActiveWrites, 1)
})

test('cancelAndWait invalidates a write waiting behind flush', async () => {
  let releaseFlush
  let writes = 0
  const autosave = new RecoveryAutosave({
    flush: () => new Promise((resolve) => { releaseFlush = resolve }),
    write: async () => { writes += 1 },
  })

  const inFlight = autosave.runNow()
  await new Promise((resolve) => setTimeout(resolve, 0))
  const cancelled = autosave.cancelAndWait()
  releaseFlush()
  await Promise.all([inFlight, cancelled])

  assert.equal(writes, 0)
})

test('runNow flushes once before a pending debounce can fire', async () => {
  let writes = 0
  const autosave = new RecoveryAutosave({
    delay: 50,
    flush: async () => {},
    write: async () => { writes += 1 },
  })

  autosave.schedule()
  await autosave.runNow()
  await new Promise((resolve) => setTimeout(resolve, 60))

  assert.equal(writes, 1)
})

test('changing the delay reschedules a pending write without duplicating it', async () => {
  let writes = 0
  const autosave = new RecoveryAutosave({
    delay: 100,
    flush: async () => {},
    write: async () => { writes += 1 },
  })

  autosave.schedule()
  autosave.setDelay(1)
  assert.equal(autosave.getDelay(), 1)
  await new Promise((resolve) => setTimeout(resolve, 20))

  assert.equal(writes, 1)
})

test('changing the delay preserves serialization with an in-flight write', async () => {
  let releaseFirst
  let active = 0
  let maximumActive = 0
  const autosave = new RecoveryAutosave({
    delay: 1,
    flush: async () => {},
    write: async () => {
      active += 1
      maximumActive = Math.max(maximumActive, active)
      if (!releaseFirst) await new Promise((resolve) => { releaseFirst = resolve })
      active -= 1
    },
  })

  const first = autosave.runNow()
  await new Promise((resolve) => setTimeout(resolve, 0))
  autosave.setDelay(2)
  autosave.schedule()
  releaseFirst()
  await first
  await new Promise((resolve) => setTimeout(resolve, 20))

  assert.equal(maximumActive, 1)
})

test('rejects invalid configurable delays', () => {
  const autosave = new RecoveryAutosave({
    flush: async () => {},
    write: async () => {},
  })
  assert.throws(() => autosave.setDelay(-1), RangeError)
  assert.throws(() => autosave.setDelay(Number.NaN), RangeError)
})

test('a successful recovery write announces that a prior failure cleared', async () => {
  let fail = true
  const events = []
  const autosave = new RecoveryAutosave({
    flush: async () => {},
    write: async () => {
      if (fail) throw new Error('disk unavailable')
    },
    onError: () => events.push('error'),
    onSuccess: () => events.push('success'),
  })

  await assert.rejects(autosave.runNow(), /disk unavailable/)
  fail = false
  await autosave.runNow()
  assert.deepEqual(events, ['error', 'success'])
})
