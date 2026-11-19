import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { AckBatcher } from "./ackBatcher";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

test("small acks are coalesced and sent after the delay", () => {
  const send = vi.fn(() => true);
  const b = new AckBatcher(send, 1000, 16);
  b.add(100);
  b.add(200);
  expect(send).not.toHaveBeenCalled();
  vi.advanceTimersByTime(16);
  expect(send).toHaveBeenCalledTimes(1);
  expect(send).toHaveBeenCalledWith(300);
});

test("reaching the threshold flushes immediately", () => {
  const send = vi.fn(() => true);
  const b = new AckBatcher(send, 1000, 16);
  b.add(600);
  b.add(600);
  expect(send).toHaveBeenCalledWith(1200);
});

test("undeliverable acks are retained and retried", () => {
  let ready = false;
  const send = vi.fn((_n: number) => ready);
  const b = new AckBatcher(send, 1000, 16);
  b.add(50);
  vi.advanceTimersByTime(16);
  expect(send).toHaveBeenCalledTimes(1);
  ready = true;
  b.add(25);
  vi.advanceTimersByTime(16);
  expect(send).toHaveBeenLastCalledWith(75);
});

test("dispose cancels pending work", () => {
  const send = vi.fn(() => true);
  const b = new AckBatcher(send, 1000, 16);
  b.add(10);
  b.dispose();
  vi.advanceTimersByTime(100);
  expect(send).not.toHaveBeenCalled();
});
