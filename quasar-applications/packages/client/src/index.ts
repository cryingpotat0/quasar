// Conditionally import winston only in Node environment
let winston: any;
if (typeof window === 'undefined') {
    // Dynamic import for Node environment only
    const winstonModule = require('winston');
    winston = winstonModule.default;
}
import { Value } from '@sinclair/typebox/value';
import * as protocol from './protocol';
import { Buffer as BufferPolyfill } from 'buffer';
const BufferImpl = typeof Buffer !== 'undefined' ? Buffer : BufferPolyfill;
export * as protocol from './protocol';

// Conditionally import ws only in Node environment
let NodeWebSocket: typeof WebSocket | null = null;
if (typeof window === 'undefined') {
    try {
        // Use require for Node environment
        const ws = require('ws');
        NodeWebSocket = ws.default || ws;
    } catch (e) {
        // ws not available
    }
}

// Use native WebSocket for browser, Node WebSocket for Node
const WebSocketImpl = typeof window !== 'undefined' ? WebSocket : NodeWebSocket;

export type ConnectionOptions = {
    connectionType: 'new_channel' 
} | {
    connectionType: 'code',
    code: string,
} | {
    connectionType: 'channel_uuid',
    channelUuid: string,
}

class ConnectionUrl {
    constructor(private _url: string, private connectionOptions: ConnectionOptions) {}

    get url(): string {
        let url = this._url;
        if (!url.startsWith('ws://') && !url.startsWith('wss://')) {
            url = `wss://${url}`;
        }
        switch (this.connectionOptions.connectionType) {
            case 'new_channel':
                return `${url}/ws/new`;
            case 'code':
                return `${url}/ws/connect?code=${this.connectionOptions.code}`;
            case 'channel_uuid':
                return `${url}/ws/connect?id=${this.connectionOptions.channelUuid}`;
            default:
                let _: never = this.connectionOptions;
                throw new Error('Unreachable');
        }
    }
}


interface Logger {
    debug: (...message: any[]) => void;
    warn: (...message: any[]) => void;
    info: (...message: any[]) => void;
    error: (...message: any[]) => void;
}

export interface QuasarClientOptions {
    url: string;
    connectionOptions: ConnectionOptions;
    debug?: boolean;
    onClose: () => void;
    onError: (error: Error) => void;
    receiveData?: (message: string) => void;
    logger?: Logger
}

class PromiseWrapper<T> {
    public promise: Promise<T>;
    private _resolve!: (value: T) => void;
    private _reject!: (error: Error) => void;

    constructor() {
        this.promise = new Promise((resolve, reject) => {
            this._resolve = resolve;
            this._reject = reject;
        });
    }

    public resolve(value: T): void {
        this._resolve(value);
    }

    public reject(error: Error): void {
        this._reject(error);
    }
}

// Create a minimal logger interface for browsers
const createBrowserLogger = (debug: boolean): Logger => ({
    debug: debug ? console.debug.bind(console) : () => {},
    info: console.info.bind(console),
    warn: console.warn.bind(console),
    error: console.error.bind(console)
});

export class QuasarClient {
    private ws: WebSocket;
    private logger: Logger;
    private generateCodePromise: PromiseWrapper<string> | null = null;
    private connectedPromise: PromiseWrapper<void>;
    private disconnectPromise: PromiseWrapper<void> | null = null;
    private _id: string | null = null;
    private _channelUuid: string | null = null;
    private _clientIds: Set<string> = new Set();
    private _messageBuffer: string[] = [];

    constructor(private options: QuasarClientOptions) {
        // Use browser-compatible logger when in browser environment
        if (typeof window !== 'undefined') {
            this.logger = options.logger || createBrowserLogger(!!options.debug);
        } else if (winston) {
            this.logger = options.logger || winston.createLogger({
                level: options.debug ? 'debug' : 'info',
                format: winston.format.simple(),
                transports: [new winston.transports.Console()],
            });
        } else {
            // Fallback logger if neither winston nor window is available
            this.logger = createBrowserLogger(!!options.debug);
        }

        const url = new ConnectionUrl(options.url, options.connectionOptions).url;
        this.logger.debug(`Attempting to connect to WebSocket at ${url}`);

        if (!WebSocketImpl) {
            throw new Error('WebSocket implementation not available');
        }

        this.ws = new WebSocketImpl(url);
        this.connectedPromise = new PromiseWrapper();

        // Bind event listeners
        if (typeof window !== 'undefined') {
            // Browser WebSocket
            this.ws.onopen = this.handleOpen.bind(this);
            this.ws.onclose = this.handleClose.bind(this);
            this.ws.onerror = (event: Event) => this.handleError(event as any);
            this.ws.onmessage = (event: MessageEvent) => this.handleMessage(event.data);
        } else {
            // Node WebSocket
            // @ts-ignore
            this.ws.on('open', this.handleOpen.bind(this));
            // @ts-ignore
            this.ws.on('close', this.handleClose.bind(this));
            // @ts-ignore
            this.ws.on('error', this.handleError.bind(this));
            // @ts-ignore
            this.ws.on('message', this.handleMessage.bind(this));
        }
    }

    public get id(): string {
        if (!this._id) {
            throw new Error('Client is not connected');
        }
        return this._id;
    }

    public get channelUuid(): string {
        if (!this._channelUuid) {
            throw new Error('Client is not connected');
        }
        return this._channelUuid;
    }

    public get clientIds(): Set<string> {
        if (!this._clientIds.size) {
            throw new Error('Client is not connected');
        }
        return this._clientIds;
    }

    public connect(): Promise<void> {
        // TODO: handle reconnects.
        this.logger.debug('Waiting for WebSocket connection to open');
        return this.connectedPromise.promise;
    }


    private handleOpen(): void {
        this.logger.debug('Connected to Quasar server');
    }

    private handleClose(): void {
        this.logger.info('Disconnected from Quasar server');
        this.options.onClose?.();
    }

    private handleError(error: Error): void {
        this.logger.error('WebSocket error:', error);
        this.options.onError?.(error);
    }

    private handleData(data: string): void {
        if (!this.options.receiveData) {
            this._messageBuffer.push(data);
        } else {
            this.options.receiveData(data)
        }
    }

    public onMessage(callback: (message: string) => void): void {
        for (const message of this._messageBuffer) {
            callback(message);
        }
        this.options.receiveData = callback;
    }


    private handleMessage(data: any): void {
        const message = data instanceof BufferImpl ? data.toString() : data.toString();
        this.logger.debug(`Received message: ${message}`);
        // TODO: value.decode is not working with union types??
        let parsedMessage: protocol.IncomingMessage = JSON.parse(message);
        try {
            // parsedMessage = Value.Decode(protocol.IncomingMessage, message);
        } catch (error: any) {
            const errors = Value.Errors(protocol.IncomingMessage, message);
            for (const error of errors) {
                this.logger.error(`Error: ${JSON.stringify(error)}`);
            }
            throw new Error(`Failed to parse message: ${error}`);

        }
        switch (parsedMessage.type) {
            case 'generated_code':
                const code = parsedMessage.code;
                this.logger.debug(`Received generated code: ${code}`);
                if (!this.generateCodePromise) {
                    throw new Error('Received generated_code message without a pending generateCode call');
                }
                this.generateCodePromise.resolve(code);
                this.generateCodePromise = null;
                break;
            case 'data':
                this.handleData(parsedMessage.content);
                break;
            case 'connection_info':
                if (parsedMessage.protocol_version !== protocol.PROTOCOL_VERSION) {
                    throw new Error(`Unsupported protocol version: ${parsedMessage.protocol_version}`);
            }
                this._id = parsedMessage.id;
                this._channelUuid = parsedMessage.channel_uuid;
                this._clientIds = new Set(parsedMessage.client_ids);
                this.connectedPromise.resolve();
                break;
            case 'client_connected':
                this.logger.debug(`Client connected: ${parsedMessage.id}`);
                this._clientIds.add(parsedMessage.id);
                break;
            case 'client_disconnected':
                this.logger.debug(`Client disconnected: ${parsedMessage.id}`);
                this._clientIds.delete(parsedMessage.id);
                break;
            default:
                // Exhaustive matching.
                let _: never = parsedMessage;
        };
    }

    public async generateCode(): Promise<string> {
        if (this.generateCodePromise) {
            throw new Error('Another generateCode call is already in progress');
        }

        this.generateCodePromise = new PromiseWrapper();
        this.sendOutgoing({ type: 'generate_code' });
        return this.generateCodePromise.promise;
    }


    public sendData(message: string): void {
        this.logger.debug(`Sending data message: ${message}`);
        this.sendOutgoing({ type: 'data', content: message });
    }

    private sendOutgoing(message: protocol.OutgoingMessage): void {
        this.ws.send(JSON.stringify(Value.Encode(protocol.OutgoingMessage, message)));
    }

    private close(): void {
        this.disconnectPromise = new PromiseWrapper();
        this.logger.info('Closing connection');
        this.ws.close();
    }

    public disconnect(): Promise<void> {
        this.close();
        return this.disconnectPromise?.promise ?? Promise.resolve();
    }
}

