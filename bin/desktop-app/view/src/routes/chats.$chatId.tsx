import ChatPage from '@/components/chat/chat-page';
import { controlBasePath } from '@/lib/utils';
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react';

export const Route = createFileRoute('/chats/$chatId')({
  component: ChatComponent,
})

function ChatComponent() {
  const [chatId, setChatId] = useState<string>(Route.useParams().chatId);

  if (!controlBasePath) {
    throw new Error("VLLM_CONTROLS_URL is not defined");
  }

  return <ChatPage chatId={chatId} setChatId={setChatId} />;
}
