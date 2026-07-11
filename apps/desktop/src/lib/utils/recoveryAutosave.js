export class RecoveryAutosave {
  /**
   * @param {{delay?: number, flush: () => Promise<unknown>, write: () => Promise<unknown>, onError?: (error: unknown) => void}} options
   */
  constructor({ delay = 750, flush, write, onError }) {
    this.delay = delay
    this.flush = flush
    this.write = write
    this.onError = onError ?? (() => {})
    this.timer = null
    this.generation = 0
    this.tail = Promise.resolve()
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
    })
    this.tail = execution.catch((error) => {
      this.onError(error)
    })
    return execution
  }
}
