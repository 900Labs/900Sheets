import assert from 'node:assert/strict'
import test from 'node:test'
import { MutationQueue } from '../../src/lib/utils/mutationQueue.js'

test('flush waits for the immediately preceding edit before save snapshot', async () => {
  const queue = new MutationQueue()
  let workbookValue = 'old'

  queue.enqueue(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20))
    workbookValue = 'latest'
  }, async () => {})

  await queue.flush()
  const savedValue = workbookValue
  assert.equal(savedValue, 'latest')
})

test('flush rejects after a failed mutation', async () => {
  const queue = new MutationQueue()
  queue.enqueue(async () => {
    throw new Error('write failed')
  }, async () => {})
  await assert.rejects(queue.flush(), /write failed/)
})

test('a later successful mutation does not hide an unacknowledged failure', async () => {
  const queue = new MutationQueue()
  const applied = []

  queue.enqueue(async () => {
    throw new Error('first edit was not written')
  }, async () => {})
  queue.enqueue(async () => {
    applied.push('second edit')
  }, async () => {})

  await assert.rejects(queue.flush(), /first edit was not written/)
  assert.deepEqual(applied, ['second edit'])
  await assert.doesNotReject(queue.flush())
})

test('a failed queued edit prevents save from clearing dirty state', async () => {
  const queue = new MutationQueue()
  let dirty = true
  let snapshotWritten = false

  queue.enqueue(async () => {
    throw new Error('cell edit failed')
  }, async () => {})

  await assert.rejects(async () => {
    await queue.flush()
    snapshotWritten = true
    dirty = false
  }, /cell edit failed/)

  assert.equal(snapshotWritten, false)
  assert.equal(dirty, true)
})
