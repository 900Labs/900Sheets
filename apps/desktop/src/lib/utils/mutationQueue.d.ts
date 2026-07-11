export class MutationQueue {
  enqueue(operation: () => Promise<unknown>, onError: (error: unknown) => Promise<void>): void
  flush(): Promise<void>
}
