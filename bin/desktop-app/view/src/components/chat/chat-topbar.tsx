"use client";

import React, { useEffect } from "react";

import {
  ButtonIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  DotFilledIcon,
  HamburgerMenuIcon,
  InfoCircledIcon,
  StopIcon,
} from "@radix-ui/react-icons";
import { Message } from "ai/react";
import { toast } from "sonner";

import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { encodeChat, getTokenLimit } from "@/lib/token-counter";
import { basePath, controlBasePath, useHasMounted } from "@/lib/utils";
import { Sidebar } from "../sidebar";
import { ChatOptions } from "./chat-options";
import { Button } from "../ui/button";
import { stopP2pSession } from "@/api/p2p";
import { useP2PStatus } from "@/queries/p2p";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import ChatModelStatus from "./chat-model-status";

interface ChatTopbarProps {
  chatOptions: ChatOptions;
  setChatOptions: React.Dispatch<React.SetStateAction<ChatOptions>>;
  isLoading: boolean;
  chatId?: string;
  setChatId: React.Dispatch<React.SetStateAction<string>>;
  messages: Message[];
}

export default function ChatTopbar({
  chatOptions,
  setChatOptions,
  isLoading,
  chatId,
  setChatId,
  messages,
}: ChatTopbarProps) {
  const hasMounted = useHasMounted();

  const currentModel = chatOptions && chatOptions.selectedModel;
  const [tokenLimit, setTokenLimit] = React.useState<number>(4096);
  const { status } = useP2PStatus({baseControlUrl: controlBasePath});
  const [error, setError] = React.useState<string | undefined>(undefined);

  const fetchData = async () => {
    if (status) {
      setChatOptions({ ...chatOptions, selectedModel: status?.models[0]?.model });
    }
  };

  useEffect(() => {
    fetchData();
  }, [hasMounted]);

  if (!hasMounted) {
    return (
      <div className="md:w-full flex px-4 py-6 items-center gap-1 md:justify-center">
        <DotFilledIcon className="w-4 h-4 text-blue-500" />
        <span className="text-xs">Booting up..</span>
      </div>
    );
  }

  const chatTokens = messages.length > 0 ? encodeChat(messages) : 0;

  return (
    <div className="md:w-full flex px-4 py-4 items-center justify-between md:justify-center">
      <Sheet>
        <SheetTrigger>
          <div className="flex items-center gap-2">
            <HamburgerMenuIcon className="md:hidden w-5 h-5" />
          </div>
        </SheetTrigger>
        <SheetContent side="left">
          <div>
            <Sidebar
              chatId={chatId || ""}
              setChatId={setChatId}
              isCollapsed={false}
              isMobile={false}
              chatOptions={chatOptions}
              setChatOptions={setChatOptions}
            />
          </div>
        </SheetContent>
      </Sheet>

      <div className="flex justify-between items-center gap-4 w-full">
        {/* Left side */}
        <div className="gap-1 flex items-center">
          {currentModel !== undefined && status && status.models && status.models.length > 0 ? (
            <>
              {isLoading ? (
                <DotFilledIcon className="w-4 h-4 text-blue-500" />
              ) : (
                <>
                  <TooltipProvider>
                    <Tooltip>
                      <TooltipTrigger>
                        <span className="cursor-help">
                          <CheckCircledIcon className="w-4 h-4 text-green-500" />
                        </span>
                      </TooltipTrigger>
                      <TooltipContent
                        sideOffset={4}
                        className="bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 p-2 rounded-sm text-xs"
                      >
                        <p className="font-bold">Current Model</p>
                        <p className="text-gray-500">{currentModel}</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </>
              )}
              <span className="text-xs">
                {isLoading ? "Generating.." : "Ready"}
              </span>
            </>
          ) : (
            <>
              <CrossCircledIcon className="w-4 h-4 text-yellow-500" />
              <span className="text-xs">Waiting for an active model...</span>
            </>
          )}
        </div>

        {/* Center - Model Selection and Status */}
        <div className="flex items-center gap-2">
          {status && status.models && status.models.length > 0 && (
            <Select
              value={chatOptions.selectedModel}
              onValueChange={(value) => setChatOptions({ ...chatOptions, selectedModel: value })}
            >
              <SelectTrigger className="w-[180px]">
                <SelectValue placeholder="Select model" />
              </SelectTrigger>
              <SelectContent>
                {status.models.map((model) => (
                  <SelectItem key={model.model} value={model.model}>
                    <div className="flex items-center gap-2">
                      <span>{model.model}</span>
                      <TooltipProvider>
                        <Tooltip>
                          <TooltipTrigger>
                            <span className="cursor-help">
                              {model.status === 'ready' ? (
                                <CheckCircledIcon className="w-4 h-4 text-green-500" />
                              ) : (
                                <DotFilledIcon className="w-4 h-4 text-yellow-500 animate-pulse" />
                              )}
                            </span>
                          </TooltipTrigger>
                          <TooltipContent
                            sideOffset={4}
                            className="bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 p-2 rounded-sm text-xs"
                          >
                            <p className="font-bold">{model.model}</p>
                            <p className="text-gray-500">
                              {model.status === 'ready' ? 'Ready' : 'Incomplete...'}
                            </p>
                          </TooltipContent>
                        </Tooltip>
                      </TooltipProvider>
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </div>

        {/* Right side */}
        <div className="flex gap-2 items-center">
          {chatTokens > tokenLimit && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger>
                  <span>
                    <InfoCircledIcon className="w-4 h-4 text-blue-500" />
                  </span>
                </TooltipTrigger>
                <TooltipContent
                  sideOffset={4}
                  className="z-60 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 rounded-sm text-xs"
                >
                  <p className="text-gray-500">
                    Token limit exceeded. Truncating middle messages.
                  </p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
          {messages.length > 0 && (
            <span className="text-xs text-gray-500">
              {chatTokens} / {tokenLimit} token{chatTokens > 1 ? "s" : ""}
            </span>
          )}
          <ChatModelStatus />
        </div>
      </div>
    </div>
  );
}
