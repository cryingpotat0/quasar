import { useState, useEffect } from 'react'
import Excalidraw from './Excalidraw'
import { QuasarClient } from '@quasartc/client'
import './App.css' // You'll need to create this file for styling

const quasarUrl = 'ws://localhost:8080'

export default function App() {
    const [client, setClient] = useState<QuasarClient | null>(null);
    const [roomCode, setRoomCode] = useState('');
    const [channelCode, setChannelCode] = useState('');
    const [error, setError] = useState('');

    const createNewRoom = async () => {
        try {
            const client = new QuasarClient({
                url: quasarUrl,
                debug: true,
                logger: console,
                connectionOptions: {
                    connectionType: 'new_channel',
                },
                onClose: () => console.log('Disconnected from QuasarClient'),
                onError: (error: any) => setError(error.message),
                receiveData: (message: string) => console.log('Received:', message),
            });

            await client.connect();
            setClient(client);
            setChannelCode(client.channelUuid);
        } catch (e) {
            setError('Error creating room: ' + (e as Error).message);
        }
    };

    const joinRoom = async () => {
        if (!roomCode.trim()) {
            setError('Please enter a room code');
            return;
        }

        try {
            const client = new QuasarClient({
                url: quasarUrl,
                debug: true,
                logger: console,
                connectionOptions: {
                    connectionType: 'channel_uuid',
                    channelUuid: roomCode,
                },
                onClose: () => console.log('Disconnected from QuasarClient'),
                onError: (error: any) => setError(error.message),
                receiveData: (message: string) => console.log('Received:', message),
            });

            await client.connect();
            setClient(client);
            setChannelCode(client.channelUuid);
        } catch (e) {
            setError('Error joining room: ' + (e as Error).message);
        }
    };

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            client?.disconnect();
        };
    }, [client]);

    if (client) {
        return (
            <div>
                <div className="room-code-container">
                    <input 
                        type="text" 
                        value={channelCode} 
                        readOnly 
                        onClick={(e) => {
                            (e.target as HTMLInputElement).select();
                            navigator.clipboard.writeText(channelCode);
                        }}
                    />
                </div>
                <Excalidraw client={client} />
            </div>
        );
    }

    return (
        <div className="join-container">
            {error && <div className="error">{error}</div>}
            <div className="actions">
                <button onClick={createNewRoom}>Create New Room</button>
                <div className="join-room">
                    <input
                        type="text"
                        value={roomCode}
                        onChange={(e) => setRoomCode(e.target.value)}
                        placeholder="Enter room code"
                    />
                    <button onClick={joinRoom}>Join Room</button>
                </div>
            </div>
        </div>
    );
}
