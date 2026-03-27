/** JavaScript module docs. */

// Exported helper context.
export function greet(name) {
  /** Normalize before returning. */
  function normalize(value) {
    return value.trim();
  }

  return normalize(name);
}

/** Builder docs. */
export const buildGreeter = (
  prefix,
) => {
  // Reject empty prefixes.
  if (!prefix) {
    throw new Error("missing prefix");
  }

  return (name) => `${prefix}: ${name}`;
};

class Greeter {
  /** Format one name. */
  format(name) {
    // Keep the method exit.
    return `${name}!`;
  }
}
