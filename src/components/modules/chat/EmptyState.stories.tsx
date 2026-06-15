import type { Meta, StoryObj } from '@storybook/react';
import { EmptyState } from './EmptyState';

const meta: Meta<typeof EmptyState> = {
  title: 'Chat/EmptyState',
  component: EmptyState,
};
export default meta;
type Story = StoryObj<typeof EmptyState>;

export const WithContact: Story = {
  args: { contactName: 'Alice' },
};

export const NewConversation: Story = {
  args: {},
};
