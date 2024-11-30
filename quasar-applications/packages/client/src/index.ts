import WebSocket from 'ws';
import winston from 'winston';
import { Value } from '@sinclair/typebox/value';
import * as protocol from './protocol';
export * as protocol from './protocol';

export type ConnectionOptions = {
    connectionType: 'new_channel' 
} | {
    connectionType: 'code',
    code: string,
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
    receiveData: (message: string) => void;
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


export class QuasarClient {
    private ws: WebSocket;
    private logger: Logger;
    private generateCodePromise: PromiseWrapper<string> | null = null;
    private connectedPromise: PromiseWrapper<void>;
    private disconnectPromise: PromiseWrapper<void> | null = null;
    private _id: string | null = null;
    private _channelUuid: string | null = null;
    private _clientIds: Set<string> = new Set();

    constructor(private options: QuasarClientOptions) {
        this.logger = options.logger || winston.createLogger({
            level: options.debug ? 'debug' : 'info',
            format: winston.format.simple(),
            transports: [new winston.transports.Console()],
        });

        const url = new ConnectionUrl(options.url, options.connectionOptions).url;

        this.logger.debug(`Attempting to connect to WebSocket at ${url}`);
        this.ws = new WebSocket(url);
        this.connectedPromise = new PromiseWrapper();

        this.ws.on('open', this.handleOpen.bind(this));
        this.ws.on('close', this.handleClose.bind(this));
        this.ws.on('error', this.handleError.bind(this));
        this.ws.on('message', this.handleMessage.bind(this));
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


    private handleMessage(data: WebSocket.Data): void {
        const message = data.toString();
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
                this.options.receiveData(parsedMessage.content);
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

