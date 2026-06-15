import '../src/App.css'
import type { Preview } from '@storybook/react-vite'

const preview: Preview = {
  decorators: [
    (Story) => (
      <div className="dark min-h-screen bg-background text-foreground p-6">
        <Story />
      </div>
    ),
  ],
  parameters: {
    backgrounds: { disable: true },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
};

export default preview;
