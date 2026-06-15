import type { Meta, StoryObj } from '@storybook/react';
import { Input } from './input';

const meta: Meta<typeof Input> = {
  title: 'UI/Input',
  component: Input,
};
export default meta;
type Story = StoryObj<typeof Input>;

export const Default: Story = {
  args: { placeholder: 'Identifiant' },
};

export const Password: Story = {
  args: { type: 'password', placeholder: 'Votre mot de passe sécurisé' },
};

export const Disabled: Story = {
  args: { placeholder: 'Désactivé', disabled: true },
};

export const WithValue: Story = {
  args: { value: 'alice_zenth', readOnly: true },
};
