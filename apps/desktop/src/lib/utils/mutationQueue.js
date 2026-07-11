export class MutationQueue {
  #tail = Promise.resolve()
  #sequence = 0
  /** @type {Array<{ sequence: number, error: unknown }>} */
  #pendingErrors = []

  /**
   * @param {() => Promise<unknown>} operation
   * @param {(error: unknown) => Promise<void>} onError
   */
  enqueue(operation, onError) {
    const sequence = ++this.#sequence
    this.#tail = this.#tail.then(async () => {
      try {
        await operation()
      } catch (error) {
        this.#pendingErrors.push({ sequence, error })
        await onError(error)
      }
    })
  }

  async flush() {
    const boundary = this.#sequence
    const tail = this.#tail
    await tail

    const failedIndex = this.#pendingErrors.findIndex(({ sequence }) => sequence <= boundary)
    if (failedIndex >= 0) {
      const [{ error }] = this.#pendingErrors.splice(failedIndex, 1)
      throw error
    }
  }
}
