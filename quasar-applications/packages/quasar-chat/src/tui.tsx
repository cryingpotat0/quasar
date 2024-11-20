import React, { useState, useEffect } from 'react';
import { Box, Text, useInput } from 'ink';
import TextInput from 'ink-text-input';
import { QuasarClient, QuasarClientOptions } from '@quasar/client';

interface ChatMessage {
  sender: string;
  content: string;
}

export const ChatTUI: React.FC<QuasarClientOptions> = (props) => {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [client, setClient] = useState<QuasarClient | null>(null);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const newClient = new QuasarClient({
      ...props,
      onOpen: () => setConnected(true),
      onClose: () => setConnected(false),
      onDataMessage: (message) => {
        const [sender, content] = message.split(':', 2);
        setMessages((prev) => [...prev, { sender, content }]);
      },
    });
    setClient(newClient);
    return () => newClient.close();
  }, []);

  useInput((input, key) => {
    if (key.return) {
      if (client && connected) {
        client.sendDataMessage(`You:${input}`);
        setMessages((prev) => [...prev, { sender: 'You', content: input }]);
        setInput('');
      }
    }
  });

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}>
        <Text>{connected ? 'Connected' : 'Disconnected'}</Text>
      </Box>
      {messages.map((msg, index) => (
        <Text key={index}>
          {msg.sender}: {msg.content}
        </Text>
      ))}
      <TextInput
        value={input}
        onChange={setInput}
        placeholder="Type a message..."
      />
    </Box>
  );
};
