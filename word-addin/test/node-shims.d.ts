// Hand-written type surface for Node's built-in test runner (A2).
// The repo has no @types/node dependency and the v0.1 rule forbids adding
// one without a proposal; these declarations cover exactly what the
// add-in tests import.

declare module "node:test" {
  interface TestContext {
    readonly signal: AbortSignal;
  }
  export function test(name: string, fn: (t: TestContext) => void | Promise<void>): void;
  export function test(
    name: string,
    options: { skip?: boolean | string },
    fn: (t: TestContext) => void | Promise<void>,
  ): void;
}

// Minimal timer surface for the modules under test (the dist build gets
// these from the DOM lib; the test build runs without it).
declare function setTimeout(handler: () => void, timeout: number): object;
declare function clearTimeout(id: object): void;

declare module "node:assert/strict" {
  export function equal(actual: unknown, expected: unknown, message?: string): void;
  export function strictEqual(actual: unknown, expected: unknown, message?: string): void;
  export function deepStrictEqual(actual: unknown, expected: unknown, message?: string): void;
  export function ok(value: unknown, message?: string): asserts value;
  export function notEqual(actual: unknown, expected: unknown, message?: string): void;
  export function throws(fn: () => unknown, message?: string): void;
  export function match(value: string, regexp: RegExp, message?: string): void;
  export function fail(message?: string): never;
}
