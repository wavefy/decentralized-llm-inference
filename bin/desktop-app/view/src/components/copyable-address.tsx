import React from 'react';
import { Button } from "@/components/ui/button";
import { shortenAddress } from "@/lib/utils";
import { CopyIcon } from "@radix-ui/react-icons";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { toast } from 'sonner';
import { cn } from "@/lib/utils";

interface CopyableAddressProps {
  address: string;
  isCurrentUser?: boolean;
}

const CopyableAddress: React.FC<CopyableAddressProps> = ({ address, isCurrentUser = false }) => {
  const copyToClipboard = () => {
    navigator.clipboard.writeText(address);
    toast.success('Address copied to clipboard');
  };

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button 
            variant="ghost" 
            size="sm" 
            className={cn(
              "font-mono text-xs",
              isCurrentUser && "bg-primary/20 hover:bg-primary/30"
            )}
            onClick={copyToClipboard}
          >
            {shortenAddress(address)}
            <CopyIcon className="ml-2 h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <p>{isCurrentUser ? "Your address (Click to copy)" : "Click to copy"}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
};

export default CopyableAddress;
