/**
 * Batches "bytes processed" acknowledgements so we do not make one IPC call per output chunk.
 * `send` returns false when the acknowledgement could not be delivered yet (e.g. the session id is
 * not known); the bytes are kept and retried on the next flush.
 */
export class AckBatcher {
  private pending = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly send: (bytes: number) => boolean,
    private readonly threshold = 32 * 1024,
    private readonly delayMs = 16,
  ) {}

  add(bytes: number): void {
    this.pending += bytes;
    if (this.pending >= this.threshold) {
      this.flush();
    } else if (this.timer === null) {
      this.timer = setTimeout(() => this.flush(), this.delayMs);
    }
  }

  flush(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    if (this.pending > 0 && this.send(this.pending)) this.pending = 0;
  }

  dispose(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.pending = 0;
  }
}
