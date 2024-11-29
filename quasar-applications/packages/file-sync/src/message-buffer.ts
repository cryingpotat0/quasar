export class MessageBuffer {
  private messages: string[] = [];
  private waiters: ((message: string) => void)[] = [];

  public push(message: string) {
    if (this.waiters.length > 0) {
      const waiter = this.waiters.shift()!;
      waiter(message);
    } else {
      this.messages.push(message);
    }
  }

  public async next(): Promise<string> {
    if (this.messages.length > 0) {
      return this.messages.shift()!;
    }
    
    return new Promise((resolve) => {
      this.waiters.push(resolve);
    });
  }
} 