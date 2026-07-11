export class RecoveryAutosave {
  /**
   * @param {{delay?: number, flush: () => Promise<unknown>, write: () => Promise<unknown>, onError?: (error: unknown) => void, onSuccess?: () => void}} options
   */
  constructor({ delay = 750, flush, write, onError, onSuccess }) {
    this.delay = delay
    this.flush = flush
    this.write = write
    this.onError = onError ?? (() => {})
    this.onSuccess = onSuccess ?? (() => {})
    this.timer = null
    this.generation = 0
    this.tail = Promise.resolve()
  }

  /**
   * Update the debounce interval without allowing an old timer to write stale
   * state. A pending autosave is rescheduled and in-flight work remains on the
   * same serialized promise chain.
   * @param {number} delay
   */
  setDelay(delay) {
    if (!Number.isFinite(delay) || delay < 0) {
      throw new RangeError('Autosave delay must be a non-negative finite number')
    }
    const hadPendingTimer = this.timer !== null
    this.delay = delay
    if (hadPendingTimer) this.schedule()
  }

  getDelay() {
    return this.delay
  }

  schedule() {
    this.generation += 1
    const generation = this.generation
    if (this.timer !== null) clearTimeout(this.timer)
    this.timer = setTimeout(() => {
      this.timer = null
      void this.enqueue(generation).catch(() => {})
    }, this.delay)
  }

  cancel() {
    this.generation += 1
    if (this.timer !== null) {
      clearTimeout(this.timer)
      this.timer = null
    }
  }

  async cancelAndWait() {
    this.cancel()
    await this.tail
  }

  runNow() {
    this.generation += 1
    const generation = this.generation
    if (this.timer !== null) {
      clearTimeout(this.timer)
      this.timer = null
    }
    return this.enqueue(generation)
  }

  /** @param {number} generation */
  enqueue(generation) {
    const execution = this.tail.then(async () => {
      if (generation !== this.generation) return
      await this.flush()
      if (generation !== this.generation) return
      await this.write()
      this.onSuccess()
    })
    this.tail = execution.catch((error) => {
      this.onError(error)
    })
    return execution
  }
}
