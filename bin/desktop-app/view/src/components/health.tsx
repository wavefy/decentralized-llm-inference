import { registryUrl } from "@/lib/utils";
import { useSupportedModels, useSwarmHealth } from "@/queries/health";
import {
  Table,
  TableCaption,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "./ui/table";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { SwarmHealth } from "@/lib/health";
import { useMemo } from "react";
import React from "react";

const Health = () => {
  const { data: swarmHealth, isLoading: swarmHealthLoading } = useSwarmHealth({
    registryUrl,
  });

  const { data: supportedModels, isLoading: supportedModelsLoading } =
    useSupportedModels({ registryUrl });

  return (
    <div className="container p-4">
      <h1 className="text-center text-3xl font-bold mt-8 mb-8">
        Wavefy Monitor
      </h1>

      <SupportedModels
        models={supportedModels}
        swarmHealth={swarmHealth}
        isLoading={supportedModelsLoading || swarmHealthLoading}
      />
      <SwarmHealthTables
        swarmHealth={swarmHealth}
        supportedModels={supportedModels}
        isLoading={swarmHealthLoading || supportedModelsLoading}
      />
    </div>
  );
};

const isModelComplete = (nodes: SwarmHealth['nodes'], totalLayers: number) => {
  if (nodes.length === 0) return false;

  const layerRanges = nodes.map(node => ({
    start: node.info.layers.start,
    end: node.info.layers.end
  }));

  // Sort the ranges by start value
  layerRanges.sort((a, b) => a.start - b.start);

  let coveredUntil = 0;
  for (const range of layerRanges) {
    if (range.start > coveredUntil + 1) {
      // There's a gap in the coverage
      return false;
    }
    coveredUntil = Math.max(coveredUntil, range.end);
  }

  // Check if we've covered all layers
  return coveredUntil === totalLayers;
};

const SupportedModels = ({
  models,
  swarmHealth,
  isLoading,
}: {
  models: { id: string; layers: number; memory: number }[] | undefined;
  swarmHealth: SwarmHealth[] | undefined;
  isLoading: boolean;
}) => {
  if (isLoading) return <p>Loading supported models...</p>;

  return (
    <Card className="mb-8">
      <CardHeader>
        <CardTitle>Supported Models</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap items-center gap-2">
          {models?.map((model, index) => {
            const healthData = swarmHealth?.find(h => h.model === model.id);
            const complete = healthData ? isModelComplete(healthData.nodes, healthData.total_layers) : false;

            return (
              <React.Fragment key={model.id}>
                <span
                  className={`font-medium ${complete ? 'text-green-500' : 'text-yellow-500'
                    }`}
                >
                  {model.id}
                </span>
                {index < models.length - 1 && (
                  <span className="text-gray-300">•</span>
                )}
              </React.Fragment>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
};

const LayerVisualizer = ({
  start,
  end,
  total,
}: {
  start: number;
  end: number;
  total: number;
}) => {
  const dots = useMemo(() => {
    return Array.from({ length: total }, (_, i) => {
      if (i + 1 >= start && i + 1 <= end) {
        return (
          <span
            key={i}
            className="inline-block w-2 h-2 bg-green-500 rounded-full mr-1"
          />
        );
      }
      return (
        <span
          key={i}
          className="inline-block w-2 h-2 bg-gray-300 rounded-full mr-1"
        />
      );
    });
  }, [start, end, total]);

  return <div className="flex flex-wrap gap-1 mt-2">{dots}</div>;
};

const SwarmHealthTables = ({
  swarmHealth,
  supportedModels,
  isLoading,
}: {
  swarmHealth: SwarmHealth[] | undefined;
  supportedModels: { id: string; layers: number; memory: number }[] | undefined;
  isLoading: boolean;
}) => {
  if (isLoading) return <p>Loading swarm health...</p>;

  const modelMap = new Map(swarmHealth?.map(model => [model.model, model]));

  return (
    <>
      {supportedModels?.map((supportedModel) => {
        const model = modelMap.get(supportedModel.id) || { model: supportedModel.id, total_layers: supportedModel.layers, nodes: [] };
        const complete = isModelComplete(model.nodes, model.total_layers);

        return (
          <Card key={model.model} className="mb-8">
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>{model.model}</span>
                <span className={`text-sm font-normal ${complete ? 'text-green-500' : 'text-yellow-500'}`}>
                  {complete ? 'Complete' : 'Incomplete'}
                </span>
              </CardTitle>
            </CardHeader>
            <CardContent>
              <Table>
                <TableCaption>Worker Nodes for {model.model}</TableCaption>
                <TableHeader>
                  <TableRow>
                    <TableHead>Node ID</TableHead>
                    <TableHead>Layers</TableHead>
                    <TableHead>Output Tps</TableHead>
                    <TableHead>Output Tokens</TableHead>
                    <TableHead>Network Out</TableHead>
                    <TableHead>Network In</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {model.nodes.length > 0 ? (
                    model.nodes.map((node) => (
                      <TableRow key={node.id}>
                        <TableCell>{node.id}</TableCell>
                        <TableCell>
                          <div>
                            {node.info.layers.start} - {node.info.layers.end} (out of{" "}
                            {model.total_layers})
                          </div>
                          <LayerVisualizer
                            start={node.info.layers.start}
                            end={node.info.layers.end}
                            total={model.total_layers}
                          />
                        </TableCell>
                        <TableCell>{node.info.stats.token_out_tps}</TableCell>
                        <TableCell>{node.info.stats.token_out_sum}</TableCell>
                        <TableCell>{node.info.stats.network_out_bytes}</TableCell>
                        <TableCell>{node.info.stats.network_in_bytes}</TableCell>
                      </TableRow>
                    ))
                  ) : (
                    <TableRow>
                      <TableCell colSpan={2} className="text-center">
                        No active nodes for this model
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        );
      })}
    </>
  );
};

export default Health;
