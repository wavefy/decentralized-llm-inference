import React from "react";
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { useP2PStatus } from "@/queries/p2p";
import { controlBasePath } from "@/lib/utils";
import P2PStatusWidget from "../p2p-status-widget";

const ChatModelStatus: React.FC = () => {
  const { status } = useP2PStatus({ baseControlUrl: controlBasePath });

  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button variant="outline" size="sm">
          Model Status
        </Button>
      </SheetTrigger>
      <SheetContent className="p-4">
        <h2 className="text-lg font-semibold mb-4">Current Model Status</h2>
        {status?.models && status.models.length > 0 ? (
          status.models.map((model, index) => (
            <P2PStatusWidget key={index} status={model} />
          ))
        ) : (
          <p>No active models.</p>
        )}
      </SheetContent>
    </Sheet>
  );
};

export default ChatModelStatus;
