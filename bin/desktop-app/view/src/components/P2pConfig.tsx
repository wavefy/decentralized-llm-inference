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

const P2pConfigWidget: React.FC = () => {
    const [status, setStatus] = useState<P2pStatus | null>(null);
    const [selectedModel, setSelectedModel] = useState<string>('');
    const [startLayer, setStartLayer] = useState<number>(0);
    const [endLayer, setEndLayer] = useState<number>(0);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        P2pStatusService.getP2pStatus().then(setStatus);

    }, []);

    useEffect(() => {
        if (selectedModel) {
            setStartLayer(0);
            setEndLayer(MODELS[selectedModel].layers);
        }
    }, [selectedModel]);

    const handleStart = async () => {
        try {
            setError(null);
            await P2pStatusService.startP2p({
                model: selectedModel,
                from_layer: startLayer,
                to_layer: endLayer,
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

    if (!status) return <div className="text-center p-4">Loading...</div>;

    return (
        <div className="bg-gray-800 p-6 rounded-lg text-gray-300 border border-gray-700 shadow-lg">
            <h2 className="font-bold mb-6 text-2xl text-blue-400">P2P Configuration</h2>
            {error && (
                <div className="p-3 rounded-md mb-4 text-red-500">
                    <p className="font-semibold">Error:</p>
                    <p>{error}</p>
                </div>
            )}
            {status.model ? (
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
                    <select
                        value={selectedModel}
                        onChange={(e) => setSelectedModel(e.target.value)}
                        className="bg-gray-700 text-black p-3 rounded-md w-full border border-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
                    >
                        <option value="">Select a model</option>
                        {Object.keys(MODELS).map((model) => (
                            <option key={model} value={model}>
                                {model}
                            </option>
                        ))}
                    </select>
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