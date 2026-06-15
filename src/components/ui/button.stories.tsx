import type { Meta, StoryObj } from '@storybook/react';
import { Button } from './button';
import { Shield, LogIn } from 'lucide-react';

const meta: Meta<typeof Button> = {
  title: 'UI/Button',
  component: Button,
  args: { children: 'Connexion sécurisée' },
};
export default meta;
type Story = StoryObj<typeof Button>;

export const Default: Story = {};

export const Outline: Story = {
  args: { variant: 'outline', children: 'Refuser' },
};

export const Destructive: Story = {
  args: { variant: 'destructive', children: 'Supprimer le compte' },
};

export const Ghost: Story = {
  args: { variant: 'ghost', children: 'Annuler' },
};

export const WithIcon: Story = {
  args: { children: <><Shield className="h-4 w-4" /> Connexion sécurisée</> },
};

export const Loading: Story = {
  args: { children: 'Connexion en cours...', disabled: true },
};

export const Sizes: Story = {
  render: () => (
    <div className="flex items-center gap-3">
      <Button size="sm">Petit</Button>
      <Button size="default">Normal</Button>
      <Button size="lg">Grand</Button>
      <Button size="icon"><LogIn /></Button>
    </div>
  ),
};

export const AllVariants: Story = {
  render: () => (
    <div className="flex flex-wrap gap-3">
      <Button variant="default">Default</Button>
      <Button variant="outline">Outline</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="ghost">Ghost</Button>
      <Button variant="destructive">Destructive</Button>
      <Button variant="link">Link</Button>
    </div>
  ),
};
