import { useEffect, useState } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'
import { QuasarClient } from '@quasartc/client'

function App() {
  const [count, setCount] = useState({
      count: 0,
      sequenceNumber: 0,
  })
  const [_client, setClient] = useState<QuasarClient | null>(null);
  const [messages, setMessages] = useState<string[]>([]);
  useEffect(() => {
      const client = new QuasarClient({
          url: 'quasar-connect.cryingpotato.com',
          debug: true,
          logger: console,
          connectionOptions: {
              connectionType: 'new_channel',
          },
          onClose: () => console.log('Disconnected from QuasarClient'),
          onError: (error: any) => console.error('Error:', error),
          receiveData: (message: string) => setMessages((messages) => [...messages, message]),
      });

        (async () => {
            try {
                await client.connect();
                setClient(client);
                await new Promise((resolve) => setTimeout(resolve, 1000));
                // setInterval(() => {
                //     if (client) {
                //         client.sendData('Hello from React');
                //     }
                // }, 1000)
            } catch (e) {
                console.error('Error connecting to client', e);
            }
        })();

        return () => {
            try {
                console.log('Closing client', client.id);
            } catch (e) {
                console.error('Closing client')
            } finally {
                client.disconnect();
                setClient(null);
            }
        };


  }, []);


  useEffect(() => {
      console.log('messages', messages);
  }, [messages]);

  return (
    <>
      <div>
        <a href="https://vite.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React</h1>
      <div className="card">
        <button onClick={() => setCount((count) => ({ sequenceNumber: count.sequenceNumber + 1, count: count.count + 1}) )}>
          count is {count.count}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </>
  )
}

export default App
