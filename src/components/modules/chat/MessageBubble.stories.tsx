import type { Meta, StoryObj } from '@storybook/react';
import { MessageBubble, type Message } from './MessageBubble';

const meta: Meta<typeof MessageBubble> = {
  title: 'Chat/MessageBubble',
  component: MessageBubble,
};
export default meta;
type Story = StoryObj<typeof MessageBubble>;

const base = new Date('2025-05-31T20:00:00');

const sentMsg: Message = {
  id: '1',
  content: 'Salut ! Tu as reçu mon message ?',
  sender: 'user',
  timestamp: base,
  status: 'read',
};

const receivedMsg: Message = {
  id: '2',
  content: 'Oui, tout est chiffré de bout en bout.',
  sender: 'assistant',
  timestamp: base,
  senderName: 'Alice',
  status: 'delivered',
};

export const Sent: Story = {
  args: { message: sentMsg },
};

export const Received: Story = {
  args: { message: receivedMsg },
};

export const Sending: Story = {
  args: { message: { ...sentMsg, status: 'sending', content: 'Message en cours d\'envoi...' } },
};

export const Error: Story = {
  args: { message: { ...sentMsg, status: 'error', content: 'Échec de l\'envoi.' } },
};

export const EmojiOnly: Story = {
  args: { message: { ...sentMsg, content: '🔐🚀✨' } },
};

export const WithReply: Story = {
  args: {
    message: {
      ...sentMsg,
      content: 'Exactement !',
      replyTo: { id: '2', content: 'Oui, tout est chiffré de bout en bout.', senderName: 'Alice' },
    },
  },
};

export const SystemMessage: Story = {
  args: {
    message: {
      id: 'sys',
      content: 'La conversation a démarré',
      sender: 'system',
      timestamp: base,
    },
  },
};

export const Conversation: Story = {
  render: () => (
    <div className="flex flex-col gap-1 bg-gray-950 p-4 rounded-xl max-w-xl">
      <MessageBubble message={receivedMsg} isFirstInGroup isLastInGroup />
      <MessageBubble message={{ ...sentMsg, content: 'Oui reçu ! Comment ça marche ?' }} isFirstInGroup isLastInGroup />
      <MessageBubble message={{ ...receivedMsg, id: '3', content: 'Dilithium + X3DH, post-quantique.' }} isFirstInGroup={false} isLastInGroup />
      <MessageBubble message={{ ...receivedMsg, id: '4', content: 'Personne d\'autre ne peut lire ça 🔐' }} isFirstInGroup={false} isLastInGroup />
    </div>
  ),
};
