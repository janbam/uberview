/** TypeScript API docs. */
export interface Service {
  run(input: string): string;
  readonly version: string;
}

export type Result<T> =
  | { ok: true; value: T }
  | { ok: false; error: string };

export namespace helpers {
  export const format = (
    prefix: string,
  ) => {
    // Return the formatted label.
    return (value: string): string => `${prefix}:${value}`;
  };
}

export class Worker {
  readonly kind: string = "worker";

  execute(
    input: string,
  ): string {
    // Keep the nested helper.
    function normalize(value: string): string {
      return value.trim();
    }

    return normalize(input);
  }
}
