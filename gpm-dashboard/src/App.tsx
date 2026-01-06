import { useState, useEffect, useCallback, useRef } from "react";
import "./App.css";

// API base URL - relative path, proxied by vite dev server or same-origin in production
const API_BASE = "/api";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from "chart.js";
import { Line } from "react-chartjs-2";
import { format } from "date-fns";

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
  Filler
);

// ============= Type Definitions =============

// Downsample data to reduce chart points for better performance
function downsampleData<T>(data: T[], maxPoints: number): T[] {
  if (data.length <= maxPoints) return data;

  const step = Math.ceil(data.length / maxPoints);
  return data.filter((_, index) => index % step === 0);
}

// Helper function to call the REST API
async function apiCall<T>(endpoint: string, params?: Record<string, string | number>): Promise<T> {
  const url = new URL(`${API_BASE}${endpoint}`, window.location.origin);
  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      url.searchParams.append(key, String(value));
    });
  }

  const response = await fetch(url.toString());
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: "Unknown error" }));
    throw new Error(error.error || `API error: ${response.status}`);
  }
  return response.json() as Promise<T>;
}

interface GpuMetricData {
  timestamp: string;
  gpu_id: number;
  name: string;
  utilization_gpu: number;
  utilization_memory: number;
  memory_used_mb: number;
  memory_total_mb: number;
  temperature: number;
  power_usage: number;
  memory_percent: number;
}

interface DashboardInfo {
  gpu_count: number;
  database_path: string;
  config_path: string;
  has_gpu_monitor: boolean;
}

interface ChartDataResponse {
  labels: string[];
  utilization_gpu: number[];
  utilization_memory: number[];
  memory_percent: number[];
  temperature: number[];
  power_usage: number[];
}

interface LlmSession {
  id: string;
  start_time: string;
  end_time: string | null;
  model: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  tokens_per_second: number;
  time_to_first_token_ms: number | null;
  time_per_output_token_ms: number | null;
}

// ============= Components =============

function GaugeMeter({ value, max, label, unit, color }: { value: number; max: number; label: string; unit: string; color: string }) {
  const percentage = Math.min((value / max) * 100, 100);
  const circumference = 2 * Math.PI * 45;
  const offset = circumference - (percentage / 100) * circumference;

  return (
    <div className="gauge-container">
      <svg className="gauge" width="120" height="120">
        <circle
          className="gauge-bg"
          cx="60"
          cy="60"
          r="45"
          fill="none"
          stroke="#333"
          strokeWidth="8"
        />
        <circle
          className="gauge-fill"
          cx="60"
          cy="60"
          r="45"
          fill="none"
          stroke={color}
          strokeWidth="8"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          transform="rotate(-90 60 60)"
        />
        <text x="60" y="55" textAnchor="middle" className="gauge-value">
          {Math.round(value)}
        </text>
        <text x="60" y="75" textAnchor="middle" className="gauge-unit">
          {unit}
        </text>
      </svg>
      <div className="gauge-label">{label}</div>
    </div>
  );
}

function MetricCard({ title, value, unit, icon }: { title: string; value: number | string; unit: string; icon: string }) {
  return (
    <div className="metric-card">
      <div className="metric-icon">{icon}</div>
      <div className="metric-content">
        <div className="metric-title">{title}</div>
        <div className="metric-value">
          {value} <span className="metric-unit">{unit}</span>
        </div>
      </div>
    </div>
  );
}

function GpuPanel({ metrics, gpuId }: { metrics: GpuMetricData | null; gpuId: number }) {
  if (!metrics) {
    return <div className="gpu-panel loading">Loading GPU {gpuId} data...</div>;
  }

  const tempColor = metrics.temperature > 85 ? "#ef4444" : metrics.temperature > 70 ? "#f59e0b" : "#22c55e";
  const utilColor = metrics.utilization_gpu > 95 ? "#ef4444" : metrics.utilization_gpu > 80 ? "#f59e0b" : "#3b82f6";
  const memColor = metrics.memory_percent > 90 ? "#ef4444" : metrics.memory_percent > 75 ? "#f59e0b" : "#8b5cf6";

  return (
    <div className="gpu-panel">
      <div className="gpu-header">
        <h2>GPU {gpuId}</h2>
        <span className="gpu-name">{metrics.name}</span>
      </div>

      <div className="gauges-row">
        <GaugeMeter
          value={metrics.utilization_gpu}
          max={100}
          label="GPU Util"
          unit="%"
          color={utilColor}
        />
        <GaugeMeter
          value={metrics.memory_percent}
          max={100}
          label="Memory"
          unit="%"
          color={memColor}
        />
        <GaugeMeter
          value={metrics.temperature}
          max={100}
          label="Temperature"
          unit="°C"
          color={tempColor}
        />
        <GaugeMeter
          value={metrics.power_usage}
          max={350}
          label="Power"
          unit="W"
          color="#06b6d4"
        />
      </div>

      <div className="metrics-grid">
        <MetricCard
          title="Memory Used"
          value={metrics.memory_used_mb.toFixed(0)}
          unit="MB"
          icon=""
        />
        <MetricCard
          title="Memory Total"
          value={metrics.memory_total_mb.toFixed(0)}
          unit="MB"
          icon=""
        />
        <MetricCard
          title="Memory Util"
          value={metrics.utilization_memory}
          unit="%"
          icon=""
        />
      </div>
    </div>
  );
}

function SingleMetricChart({
  chartData,
  title,
  dataKey,
  color,
  unit,
  min = 0,
  max,
  timeRange
}: {
  chartData: ChartDataResponse | null;
  title: string;
  dataKey: keyof ChartDataResponse;
  color: string;
  unit: string;
  min?: number;
  max?: number;
  timeRange: number;
}) {
  if (!chartData || chartData.labels.length === 0) {
    return (
      <div className="chart-container">
        <div className="chart-header">
          <h3>{title}</h3>
        </div>
        <div className="chart-placeholder">No data available</div>
      </div>
    );
  }

  // Determine max points based on time range for performance
  const maxPoints = timeRange <= 1 ? 200 : timeRange <= 6 ? 300 : 400;

  const dataValues = chartData[dataKey] as number[];
  const downsampledLabels = downsampleData(chartData.labels, maxPoints);
  const downsampledValues = downsampleData(dataValues, maxPoints);
  const currentValue = dataValues[dataValues.length - 1];
  const bgColor = color.replace(')', ', 0.1)').replace('rgb', 'rgba');

  // Calculate stats
  const minValue = Math.min(...dataValues);
  const maxValue = Math.max(...dataValues);
  const avgValue = dataValues.reduce((a, b) => a + b, 0) / dataValues.length;

  // Calculate trend (compare last 10% with previous 10%)
  const sampleSize = Math.max(5, Math.floor(dataValues.length * 0.1));
  const recent = dataValues.slice(-sampleSize);
  const previous = dataValues.slice(-sampleSize * 2, -sampleSize);
  const recentAvg = recent.reduce((a, b) => a + b, 0) / recent.length;
  const previousAvg = previous.length > 0 ? previous.reduce((a, b) => a + b, 0) / previous.length : recentAvg;
  const trendChange = previousAvg > 0 ? ((recentAvg - previousAvg) / previousAvg) * 100 : 0;
  const trend = trendChange > 5 ? "up" : trendChange < -5 ? "down" : "neutral";

  const data = {
    labels: downsampledLabels.map((t) => format(new Date(t), "HH:mm:ss")),
    datasets: [
      {
        label: `${title} (${unit})`,
        data: downsampledValues,
        borderColor: color,
        backgroundColor: bgColor,
        fill: true,
        tension: 0.4,
      },
    ],
  };

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: "index" as const,
      intersect: false,
    },
    plugins: {
      legend: {
        display: false,
      },
      tooltip: {
        enabled: true,
        backgroundColor: "rgba(17, 24, 39, 0.9)",
        titleColor: "#f9fafb",
        bodyColor: "#d1d5db",
        borderColor: "#374151",
        borderWidth: 1,
        padding: 10,
        displayColors: false,
        callbacks: {
          label: function(context: any) {
            return `${context.parsed.y.toFixed(1)} ${unit}`;
          },
          title: function(context: any) {
            return context[0]?.label || "";
          },
        },
      },
    },
    scales: {
      x: {
        ticks: { color: "#9ca3af", maxTicksLimit: 6 },
        grid: { color: "#374151" },
      },
      y: {
        ticks: { color: "#9ca3af" },
        grid: { color: "#374151" },
        min: min,
        max: max,
      },
    },
  };

  return (
    <div className="chart-container">
      <div className="chart-header">
        <h3>{title}</h3>
        <div className="chart-value-group">
          <span className="chart-current-value" style={{ color }}>
            {typeof currentValue === 'number' ? currentValue.toFixed(1) : currentValue} {unit}
          </span>
          <span className={`chart-trend chart-trend-${trend}`}>
            {trend === "up" ? "▲" : trend === "down" ? "▼" : "─"}
            {Math.abs(trendChange).toFixed(0)}%
          </span>
        </div>
      </div>
      <div className="chart-wrapper">
        <Line data={data} options={options} />
      </div>
      <div className="chart-stats">
        <span className="chart-stat">Min: {minValue.toFixed(0)}</span>
        <span className="chart-stat">Avg: {avgValue.toFixed(0)}</span>
        <span className="chart-stat">Max: {maxValue.toFixed(0)}</span>
      </div>
    </div>
  );
}

function LlmPanel({ sessions }: { sessions: LlmSession[] }) {
  if (sessions.length === 0) {
    return (
      <div className="llm-panel">
        <h3>LLM Sessions</h3>
        <div className="llm-empty">
          <p>No LLM sessions recorded yet.</p>
          <p className="llm-hint">Use Ollama through the GPM proxy (port 11434) to track sessions.</p>
        </div>
      </div>
    );
  }

  // Calculate aggregated stats
  const totalSessions = sessions.length;
  const avgTps = sessions.reduce((sum, s) => sum + s.tokens_per_second, 0) / totalSessions;
  const avgTtft = sessions.filter(s => s.time_to_first_token_ms).reduce((sum, s) => sum + (s.time_to_first_token_ms || 0), 0) / sessions.filter(s => s.time_to_first_token_ms).length || 0;
  const totalTokens = sessions.reduce((sum, s) => sum + s.total_tokens, 0);

  // Calculate average session duration
  const completedSessions = sessions.filter(s => s.end_time);
  const avgDuration = completedSessions.length > 0
    ? completedSessions.reduce((sum, s) => {
        const start = new Date(s.start_time).getTime();
        const end = new Date(s.end_time!).getTime();
        return sum + (end - start);
      }, 0) / completedSessions.length / 1000
    : 0;

  // Get unique models and find best performing
  const models = [...new Set(sessions.map(s => s.model))];
  const modelStats = models.map(model => {
    const modelSessions = sessions.filter(s => s.model === model);
    const avgTps = modelSessions.reduce((sum, s) => sum + s.tokens_per_second, 0) / modelSessions.length;
    return { model, avgTps, count: modelSessions.length };
  });
  const bestModel = modelStats.sort((a, b) => b.avgTps - a.avgTps)[0];

  return (
    <div className="llm-panel">
      <h3>LLM Sessions (Last 24h)</h3>

      <div className="llm-stats-grid llm-stats-grid-6">
        <div className="llm-stat-card">
          <div className="llm-stat-value">{totalSessions}</div>
          <div className="llm-stat-label">Sessions</div>
        </div>
        <div className="llm-stat-card">
          <div className="llm-stat-value">{avgTps.toFixed(1)}</div>
          <div className="llm-stat-label">Avg TPS</div>
        </div>
        <div className="llm-stat-card">
          <div className="llm-stat-value">{avgTtft.toFixed(0)}</div>
          <div className="llm-stat-label">Avg TTFT (ms)</div>
        </div>
        <div className="llm-stat-card">
          <div className="llm-stat-value">{(totalTokens / 1000).toFixed(1)}k</div>
          <div className="llm-stat-label">Total Tokens</div>
        </div>
        <div className="llm-stat-card">
          <div className="llm-stat-value">{avgDuration > 0 ? (avgDuration < 60 ? `${avgDuration.toFixed(0)}s` : `${(avgDuration / 60).toFixed(1)}m`) : '-'}</div>
          <div className="llm-stat-label">Avg Duration</div>
        </div>
        <div className="llm-stat-card">
          <div className="llm-stat-value llm-best-model" title={`${bestModel.model}: ${bestModel.avgTps.toFixed(1)} TPS`}>
            {bestModel.model.split(':')[0]}
          </div>
          <div className="llm-stat-label">Best Model</div>
        </div>
      </div>

      <div className="llm-models">
        <span className="llm-models-label">Models:</span>
        {modelStats.map(({ model, avgTps, count }) => (
          <span key={model} className="llm-model-tag" title={`${avgTps.toFixed(1)} TPS, ${count} sessions`}>
            {model} ({avgTps.toFixed(0)} TPS)
          </span>
        ))}
      </div>

      <div className="llm-sessions-list">
        <h4>Recent Sessions</h4>
        <table className="llm-table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Model</th>
              <th>Tokens</th>
              <th>TPS</th>
              <th>TTFT</th>
              <th>Duration</th>
            </tr>
          </thead>
          <tbody>
            {sessions.slice(0, 10).map(s => {
              const duration = s.end_time
                ? (new Date(s.end_time).getTime() - new Date(s.start_time).getTime()) / 1000
                : null;
              return (
                <tr key={s.id}>
                  <td>{format(new Date(s.start_time), "HH:mm:ss")}</td>
                  <td className="llm-model-cell">{s.model}</td>
                  <td>{s.total_tokens}</td>
                  <td>{s.tokens_per_second.toFixed(1)}</td>
                  <td>{s.time_to_first_token_ms ? `${s.time_to_first_token_ms}ms` : '-'}</td>
                  <td>{duration ? (duration < 60 ? `${duration.toFixed(0)}s` : `${(duration / 60).toFixed(1)}m`) : '-'}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ============= Main App =============

function App() {
  const [_dashboardInfo, setDashboardInfo] = useState<DashboardInfo | null>(null);
  const [realtimeMetrics, setRealtimeMetrics] = useState<GpuMetricData[]>([]);
  const [chartData, setChartData] = useState<Record<number, ChartDataResponse>>({});
  const [llmSessions, setLlmSessions] = useState<LlmSession[]>([]);
  const [selectedGpu, setSelectedGpu] = useState<number>(0);
  const [timeRange, setTimeRange] = useState<number>(1); // hours
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<number | null>(null);

  // Fetch dashboard info on mount
  useEffect(() => {
    async function fetchInfo() {
      try {
        const info: DashboardInfo = await apiCall<DashboardInfo>("/info");
        setDashboardInfo(info);
      } catch (e) {
        setError(String(e));
      }
    }
    fetchInfo();
  }, []);

  // Fetch real-time metrics
  const fetchRealtimeMetrics = useCallback(async () => {
    try {
      const metrics: GpuMetricData[] = await apiCall<GpuMetricData[]>("/realtime");
      setRealtimeMetrics(metrics);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch chart data
  const fetchChartData = useCallback(async () => {
    try {
      const data: ChartDataResponse = await apiCall<ChartDataResponse>("/chart", {
        gpu_id: selectedGpu,
        hours: timeRange,
      });
      setChartData((prev) => ({ ...prev, [selectedGpu]: data }));
    } catch (e) {
      console.error("Failed to fetch chart data:", e);
    }
  }, [selectedGpu, timeRange]);

  // Fetch LLM sessions
  const fetchLlmSessions = useCallback(async () => {
    try {
      const now = new Date();
      const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000);
      const sessions: LlmSession[] = await apiCall<LlmSession[]>("/llm-sessions", {
        start_date: yesterday.toISOString(),
        end_date: now.toISOString(),
      });
      setLlmSessions(sessions);
    } catch (e) {
      console.error("Failed to fetch LLM sessions:", e);
    }
  }, []);

  // Initial data fetch
  useEffect(() => {
    fetchRealtimeMetrics();
  }, [fetchRealtimeMetrics]);

  // Set up polling for real-time metrics
  useEffect(() => {
    intervalRef.current = window.setInterval(() => {
      fetchRealtimeMetrics();
    }, 500);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [fetchRealtimeMetrics]);

  // Fetch chart data when GPU selection or time range changes
  useEffect(() => {
    fetchChartData();
  }, [fetchChartData]);

  // Fetch LLM sessions periodically
  useEffect(() => {
    fetchLlmSessions();
    const llmInterval = window.setInterval(fetchLlmSessions, 5000);
    return () => clearInterval(llmInterval);
  }, [fetchLlmSessions]);

  // Get metrics for selected GPU
  const selectedGpuMetrics = realtimeMetrics.find((m) => m.gpu_id === selectedGpu) || null;

  if (loading) {
    return (
      <div className="app-container">
        <div className="loading">Loading dashboard...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="app-container">
        <div className="error">
          <h2>Error</h2>
          <p>{error}</p>
          <p>Make sure the GPM service is running and collecting metrics.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-title">
          <h1>GPM - GPU & LLM Monitor</h1>
          <span className="live-indicator">
            <span className="live-dot"></span>
            LIVE
          </span>
        </div>
        <div className="header-controls">
          <select
            value={selectedGpu}
            onChange={(e) => setSelectedGpu(Number(e.target.value))}
            className="gpu-selector"
          >
            {realtimeMetrics.map((m) => (
              <option key={m.gpu_id} value={m.gpu_id}>
                GPU {m.gpu_id} - {m.name}
              </option>
            ))}
          </select>
          <select
            value={timeRange}
            onChange={(e) => setTimeRange(Number(e.target.value))}
            className="time-selector"
          >
            <option value={1}>Last 1 hour</option>
            <option value={6}>Last 6 hours</option>
            <option value={24}>Last 24 hours</option>
          </select>
        </div>
      </header>

      <main className="app-main">
        {/* Real-time GPU panel for selected GPU */}
        <GpuPanel metrics={selectedGpuMetrics} gpuId={selectedGpu} />

        {/* Historical charts */}
        <div className="charts-grid four-charts">
          <SingleMetricChart
            chartData={chartData[selectedGpu] || null}
            title={`GPU Utilization (${timeRange}h)`}
            dataKey="utilization_gpu"
            color="rgb(59, 130, 246)"
            unit="%"
            max={100}
            timeRange={timeRange}
          />
          <SingleMetricChart
            chartData={chartData[selectedGpu] || null}
            title={`Memory Utilization (${timeRange}h)`}
            dataKey="utilization_memory"
            color="rgb(139, 92, 246)"
            unit="%"
            max={100}
            timeRange={timeRange}
          />
          <SingleMetricChart
            chartData={chartData[selectedGpu] || null}
            title={`Temperature (${timeRange}h)`}
            dataKey="temperature"
            color="rgb(239, 68, 68)"
            unit="°C"
            max={100}
            timeRange={timeRange}
          />
          <SingleMetricChart
            chartData={chartData[selectedGpu] || null}
            title={`Power Usage (${timeRange}h)`}
            dataKey="power_usage"
            color="rgb(6, 182, 212)"
            unit="W"
            timeRange={timeRange}
          />
        </div>

        {/* LLM Sessions Panel */}
        <LlmPanel sessions={llmSessions} />

        {/* All GPUs summary */}
        {realtimeMetrics.length > 1 && (
          <div className="all-gpus-summary">
            <h3>All GPUs</h3>
            <div className="gpus-grid">
              {realtimeMetrics.map((m) => (
                <div
                  key={m.gpu_id}
                  className={`gpu-summary-card ${m.gpu_id === selectedGpu ? "active" : ""}`}
                  onClick={() => setSelectedGpu(m.gpu_id)}
                >
                  <div className="gpu-summary-name">GPU {m.gpu_id}</div>
                  <div className="gpu-summary-stats">
                    <span>{m.utilization_gpu}% util</span>
                    <span>{m.temperature}°C</span>
                    <span>{m.power_usage}W</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </main>

      <footer className="app-footer">
        <span>Updating every 0.5 seconds</span>
        {selectedGpuMetrics && (
          <span>Last update: {new Date(selectedGpuMetrics.timestamp).toLocaleTimeString()}</span>
        )}
      </footer>
    </div>
  );
}

export default App;
