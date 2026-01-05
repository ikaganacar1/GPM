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

  const tempColor = metrics.temperature > 80 ? "#ef4444" : metrics.temperature > 60 ? "#f59e0b" : "#22c55e";
  const utilColor = metrics.utilization_gpu > 90 ? "#ef4444" : metrics.utilization_gpu > 70 ? "#f59e0b" : "#3b82f6";

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
          color="#8b5cf6"
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

function MetricsChart({ chartData, title }: { chartData: ChartDataResponse | null; title: string }) {
  if (!chartData || chartData.labels.length === 0) {
    return (
      <div className="chart-container">
        <h3>{title}</h3>
        <div className="chart-placeholder">No data available</div>
      </div>
    );
  }

  const data = {
    labels: chartData.labels.map((t) => format(new Date(t), "HH:mm:ss")),
    datasets: [
      {
        label: "GPU Utilization (%)",
        data: chartData.utilization_gpu,
        borderColor: "#3b82f6",
        backgroundColor: "rgba(59, 130, 246, 0.1)",
        fill: true,
        tension: 0.4,
      },
      {
        label: "Memory Utilization (%)",
        data: chartData.utilization_memory,
        borderColor: "#8b5cf6",
        backgroundColor: "rgba(139, 92, 246, 0.1)",
        fill: true,
        tension: 0.4,
      },
    ],
  };

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        labels: { color: "#9ca3af" },
      },
    },
    scales: {
      x: {
        ticks: { color: "#9ca3af", maxTicksLimit: 8 },
        grid: { color: "#374151" },
      },
      y: {
        ticks: { color: "#9ca3af" },
        grid: { color: "#374151" },
        min: 0,
        max: 100,
      },
    },
  };

  return (
    <div className="chart-container">
      <h3>{title}</h3>
      <div className="chart-wrapper">
        <Line data={data} options={options} />
      </div>
    </div>
  );
}

function TemperatureChart({ chartData }: { chartData: ChartDataResponse | null }) {
  if (!chartData || chartData.labels.length === 0) {
    return (
      <div className="chart-container">
        <h3>Temperature & Power</h3>
        <div className="chart-placeholder">No data available</div>
      </div>
    );
  }

  const data = {
    labels: chartData.labels.map((t) => format(new Date(t), "HH:mm:ss")),
    datasets: [
      {
        label: "Temperature (°C)",
        data: chartData.temperature,
        borderColor: "#ef4444",
        backgroundColor: "rgba(239, 68, 68, 0.1)",
        yAxisID: "y",
        fill: true,
        tension: 0.4,
      },
      {
        label: "Power (W)",
        data: chartData.power_usage,
        borderColor: "#06b6d4",
        backgroundColor: "rgba(6, 182, 212, 0.1)",
        yAxisID: "y1",
        fill: true,
        tension: 0.4,
      },
    ],
  };

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        labels: { color: "#9ca3af" },
      },
    },
    scales: {
      x: {
        ticks: { color: "#9ca3af", maxTicksLimit: 8 },
        grid: { color: "#374151" },
      },
      y: {
        type: "linear" as const,
        display: true,
        position: "left" as const,
        ticks: { color: "#ef4444" },
        grid: { color: "#374151" },
        title: { display: true, text: "Temperature (°C)", color: "#ef4444" },
      },
      y1: {
        type: "linear" as const,
        display: true,
        position: "right" as const,
        ticks: { color: "#06b6d4" },
        grid: { drawOnChartArea: false },
        title: { display: true, text: "Power (W)", color: "#06b6d4" },
      },
    },
  };

  return (
    <div className="chart-container">
      <h3>Temperature & Power</h3>
      <div className="chart-wrapper">
        <Line data={data} options={options} />
      </div>
    </div>
  );
}

// ============= Main App =============

function App() {
  const [dashboardInfo, setDashboardInfo] = useState<DashboardInfo | null>(null);
  const [realtimeMetrics, setRealtimeMetrics] = useState<GpuMetricData[]>([]);
  const [chartData, setChartData] = useState<Record<number, ChartDataResponse>>({});
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
        <h1>GPM - GPU & LLM Monitor</h1>
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
        <div className="charts-grid">
          <MetricsChart chartData={chartData[selectedGpu] || null} title={`GPU Utilization - Last ${timeRange}h`} />
          <TemperatureChart chartData={chartData[selectedGpu] || null} />
        </div>

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
