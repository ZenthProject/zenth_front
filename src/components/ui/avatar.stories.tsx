import type { Meta, StoryObj } from '@storybook/react';
import { Avatar, AvatarFallback, AvatarImage } from './avatar';

const meta: Meta = {
  title: 'UI/Avatar',
};
export default meta;

export const WithFallback: StoryObj = {
  render: () => (
    <div className="flex items-center gap-4">
      <Avatar className="h-8 w-8">
        <AvatarFallback className="bg-gradient-to-br from-indigo-500 to-purple-600 text-white text-xs">AL</AvatarFallback>
      </Avatar>
      <Avatar className="h-10 w-10">
        <AvatarFallback className="bg-gradient-to-br from-indigo-500 to-purple-600 text-white text-sm">AL</AvatarFallback>
      </Avatar>
      <Avatar className="h-14 w-14">
        <AvatarFallback className="bg-gradient-to-br from-indigo-500 to-purple-600 text-white text-base">AL</AvatarFallback>
      </Avatar>
    </div>
  ),
};

export const WithImage: StoryObj = {
  render: () => (
    <Avatar className="h-10 w-10">
      <AvatarImage src="https://github.com/shadcn.png" alt="shadcn" />
      <AvatarFallback>SC</AvatarFallback>
    </Avatar>
  ),
};

export const WithOnlineIndicator: StoryObj = {
  render: () => (
    <div className="relative w-fit">
      <Avatar className="h-10 w-10 ring-2 ring-gray-700">
        <AvatarFallback className="bg-gradient-to-br from-indigo-500 to-purple-600 text-white text-sm">AL</AvatarFallback>
      </Avatar>
      <span className="absolute bottom-0 right-0 h-3 w-3 bg-emerald-500 border-2 border-gray-900 rounded-full" />
    </div>
  ),
};
