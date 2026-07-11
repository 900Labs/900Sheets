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
