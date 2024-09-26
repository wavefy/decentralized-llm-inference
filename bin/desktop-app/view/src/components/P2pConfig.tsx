import React, { useEffect, useState } from 'react';
import { P2pStatus, P2pStatusService } from '../service/P2pStatusService';

const MODELS: any = {
    "phi3": {
        layers: 32,
        memory: 4,
    },
    "gpt2": {
        layers: 24,
        memory: 3,
    }
};

const MAX_MEMORY_OPTIONS = [1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 24, 32, 48, 64];

const P2pConfigWidget: React.FC = () => {
    const [status, setStatus] = useState<P2pStatus | null>(null);
    const [selectedModel, setSelectedModel] = useState<string>('');
    const [maxMemory, setMaxMemory] = useState<number>(8);
    const [warning, setWarning] = useState<string | null>(null);
    const [startLayer, setStartLayer] = useState<number>(0);
    const [endLayer, setEndLayer] = useState<number>(18);
    const [privateKey, setPrivateKey] = useState<string>('0x3bba41ade33b801bf3e42a080a699e73654eaf1775fb0afc5d65f5449e55d74b');
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        P2pStatusService.getP2pStatus().then(setStatus).catch((err) => {
            setError(err instanceof Error ? err.message : 'An unknown error occurred');
        });
    }, []);

    const handleSuggest = async () => {
        if (!selectedModel) {
            setWarning("Please select a model first.");
            return;
        }

        try {
            const maxLayers = Math.floor(maxMemory / MODELS[selectedModel].memory * MODELS[selectedModel].layers);
            const suggestedLayers = await P2pStatusService.suggestLayers(selectedModel, maxLayers, MODELS[selectedModel].layers);
            console.log(suggestedLayers);
            if (suggestedLayers.from_layer !== undefined && suggestedLayers.from_layer != null && suggestedLayers.to_layer !== undefined && suggestedLayers.to_layer != null) {
                setStartLayer(suggestedLayers.from_layer);
                setEndLayer(suggestedLayers.to_layer);
                setWarning(null);
            } else if (suggestedLayers.min_layers !== undefined) {
                const requiredMemory = Math.ceil(MODELS[selectedModel].memory * suggestedLayers.min_layers / MODELS[selectedModel].layers);
                setWarning(`Need at least ${requiredMemory}GB of memory for ${suggestedLayers.min_layers} layers.`);
            } else {
                setWarning("Unable to determine suggested layers.");
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : 'An unknown error occurred');
        }
    };

    const handleStart = async () => {
        try {
            setError(null);
            await P2pStatusService.startP2p({
                model: selectedModel,
                from_layer: startLayer,
                to_layer: endLayer,
                private_key: privateKey
            });
            P2pStatusService.getP2pStatus().then(setStatus);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'An unknown error occurred');
        }
    };

    const handleStop = async () => {
        try {
            setError(null);
            await P2pStatusService.stopP2p();
            P2pStatusService.getP2pStatus().then(setStatus);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'An unknown error occurred');
        }
    };

    return (
        <div className="bg-gray-800 p-6 rounded-lg text-gray-300 border border-gray-700 shadow-lg">
            <h2 className="font-bold mb-6 text-2xl text-blue-400">P2P Configuration</h2>
            {error && (
                <div className="p-3 rounded-md mb-4 text-red-500">
                    <p className="font-semibold">Error:</p>
                    <p>{error}</p>
                </div>
            )}
            {status?.model ? (
                <div className="space-y-4">
                    <div className="bg-gray-700 p-4 rounded-md">
                        <p className="font-semibold">Current model: <span className="text-blue-300">{status.model.model}</span></p>
                        <p className="font-semibold">Layers: <span className="text-blue-300">{status.model.from_layer} - {status.model.to_layer}</span></p>
                    </div>
                    <button
                        onClick={handleStop}
                        className="w-full bg-red-600 text-white px-4 py-2 rounded-md hover:bg-red-700 transition duration-300 ease-in-out"
                    >
                        Stop Model
                    </button>
                </div>
            ) : (
                <div className="space-y-4">
                    <div className="flex space-x-4 items-end">
                        <div className="flex-grow">
                            <select
                                value={selectedModel}
                                onChange={(e) => setSelectedModel(e.target.value)}
                                className="bg-gray-700 text-white p-3 rounded-md w-full border border-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
                            >
                                <option value="">Select a model</option>
                                {Object.keys(MODELS).map((model) => (
                                    <option key={model} value={model}>
                                        {model}
                                    </option>
                                ))}
                            </select>
                        </div>
                        <div className="w-1/4">
                            <select
                                value={maxMemory}
                                onChange={(e) => setMaxMemory(Number(e.target.value))}
                                className="bg-gray-700 text-white p-3 rounded-md w-full border border-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
                            >
                                {MAX_MEMORY_OPTIONS.map((mem) => (
                                    <option key={mem} value={mem}>
                                        {mem}GB
                                    </option>
                                ))}
                            </select>
                        </div>
                        <button
                            onClick={handleSuggest}
                            className="bg-green-600 text-white px-4 py-3 rounded-md hover:bg-green-700 transition duration-300 ease-in-out"
                        >
                            Calculate suggests
                        </button>
                    </div>
                    {warning && <p className="text-yellow-500">{warning}</p>}
                    {selectedModel && (
                        <>
                            <div className="bg-gray-700 p-4 rounded-md">
                                <p className="mb-2">
                                    This model has <span className="font-semibold text-blue-300">{MODELS[selectedModel].layers}</span> layers
                                    and needs <span className="font-semibold text-blue-300">{MODELS[selectedModel].memory} GB</span> of memory.
                                </p>
                                <div className="flex space-x-4">
                                    <div className="w-1/2">
                                        <label htmlFor="startLayer" className="block mb-1 text-sm">Start Layer</label>
                                        <input
                                            id="startLayer"
                                            type="number"
                                            value={startLayer}
                                            onChange={(e) => setStartLayer(Number(e.target.value))}
                                            min={0}
                                            max={MODELS[selectedModel].layers - 1}
                                            className="bg-gray-600 text-white p-2 rounded-md w-full border border-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                        />
                                    </div>
                                    <div className="w-1/2">
                                        <label htmlFor="endLayer" className="block mb-1 text-sm">End Layer</label>
                                        <input
                                            id="endLayer"
                                            type="number"
                                            value={endLayer}
                                            onChange={(e) => setEndLayer(Number(e.target.value))}
                                            min={startLayer + 1}
                                            max={MODELS[selectedModel].layers}
                                            className="bg-gray-600 text-white p-2 rounded-md w-full border border-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                        />
                                    </div>


                                </div>
                                <div className="w-full ">
                                    <label htmlFor="privateKey" className="block mb-1 text-sm">Private Key</label>
                                    <input
                                        id="privateKey"
                                        type="text"
                                        value={privateKey}
                                        onChange={(e) => setPrivateKey(e.target.value)}
                                        className="bg-gray-600 text-white p-2 rounded-md w-full border border-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                    />
                                </div>
                            </div>
                        </>
                    )}
                    <button
                        onClick={handleStart}
                        disabled={!selectedModel || startLayer >= endLayer}
                        className="w-full bg-blue-600 text-white px-4 py-3 rounded-md hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed transition duration-300 ease-in-out"
                    >
                        Start Model
                    </button>
                </div>
            )}
        </div>
    );
};

export default P2pConfigWidget;