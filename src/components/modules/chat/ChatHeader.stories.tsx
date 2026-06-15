import type { Meta, StoryObj } from '@storybook/react';
import { ChatHeader } from './ChatHeader';

const meta: Meta<typeof ChatHeader> = {
  title: 'Chat/ChatHeader',
  component: ChatHeader,
  args: {
    contactName: 'Alice',
    isEncrypted: true,
    onCall: () => {},
    onVideoCall: () => {},
    onViewProfile: () => {},
    onMuteNotifications: () => {},
    onClearChat: () => {},
    onBlockContact: () => {},
  },
};
export default meta;
type Story = StoryObj<typeof ChatHeader>;

export const Online: Story = {
  args: { isOnline: true },
};

export const Offline: Story = {
  args: { isOnline: false, lastSeen: 'Vu il y a 5 min' },
};

export const Verified: Story = {
  args: { isOnline: true, isVerified: true },
};

export const WithBackButton: Story = {
  args: { isOnline: true, onBack: () => {} },
};
