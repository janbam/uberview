/** Public greeter. */
export class Greeter {
  ask(name: string): string {
    // Validate the request.
    return name.trim();
  }
}

function localOnly(): void {
  // Hidden implementation detail.
}

function exportedLater(): void {
}

export { exportedLater };
