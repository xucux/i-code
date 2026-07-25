import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'

const requestData = [
  { time: '00:00', success: 120, failed: 8 },
  { time: '04:00', success: 80, failed: 4 },
  { time: '08:00', success: 240, failed: 16 },
  { time: '12:00', success: 380, failed: 22 },
  { time: '16:00', success: 320, failed: 18 },
  { time: '20:00', success: 280, failed: 12 },
  { time: '23:59', success: 160, failed: 6 },
]

const tokenData = [
  { day: 'Mon', input: 12000, output: 34000 },
  { day: 'Tue', input: 18000, output: 42000 },
  { day: 'Wed', input: 15000, output: 38000 },
  { day: 'Thu', input: 22000, output: 51000 },
  { day: 'Fri', input: 26000, output: 60000 },
  { day: 'Sat', input: 14000, output: 32000 },
  { day: 'Sun', input: 11000, output: 28000 },
]

const cacheData = [
  { type: 'Hit', value: 68 },
  { type: 'Miss', value: 32 },
]

export function AreaChartExample() {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <AreaChart data={requestData} margin={{ top: 8, right: 8, left: -16, bottom: 0 }}>
        <defs>
          <linearGradient id="colorSuccess" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.35} />
            <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
          </linearGradient>
          <linearGradient id="colorFailed" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="hsl(var(--destructive))" stopOpacity={0.35} />
            <stop offset="95%" stopColor="hsl(var(--destructive))" stopOpacity={0.02} />
          </linearGradient>
        </defs>
        <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" vertical={false} />
        <XAxis dataKey="time" tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} axisLine={false} tickLine={false} />
        <YAxis tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} axisLine={false} tickLine={false} />
        <Tooltip
          contentStyle={{
            backgroundColor: 'hsl(var(--card))',
            borderColor: 'hsl(var(--border))',
            borderRadius: 'var(--radius)',
            color: 'hsl(var(--card-foreground))',
          }}
        />
        <Legend wrapperStyle={{ fontSize: 12 }} />
        <Area type="monotone" dataKey="success" stroke="hsl(var(--primary))" fillOpacity={1} fill="url(#colorSuccess)" strokeWidth={2} />
        <Area type="monotone" dataKey="failed" stroke="hsl(var(--destructive))" fillOpacity={1} fill="url(#colorFailed)" strokeWidth={2} />
      </AreaChart>
    </ResponsiveContainer>
  )
}

export function LineChartExample() {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <LineChart data={tokenData} margin={{ top: 8, right: 8, left: -16, bottom: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" vertical={false} />
        <XAxis dataKey="day" tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} axisLine={false} tickLine={false} />
        <YAxis tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} axisLine={false} tickLine={false} />
        <Tooltip
          contentStyle={{
            backgroundColor: 'hsl(var(--card))',
            borderColor: 'hsl(var(--border))',
            borderRadius: 'var(--radius)',
            color: 'hsl(var(--card-foreground))',
          }}
        />
        <Legend wrapperStyle={{ fontSize: 12 }} />
        <Line type="monotone" dataKey="input" stroke="hsl(var(--secondary))" strokeWidth={2} dot={{ r: 3, fill: 'hsl(var(--secondary))' }} activeDot={{ r: 5 }} />
        <Line type="monotone" dataKey="output" stroke="hsl(var(--primary))" strokeWidth={2} dot={{ r: 3, fill: 'hsl(var(--primary))' }} activeDot={{ r: 5 }} />
      </LineChart>
    </ResponsiveContainer>
  )
}

export function BarChartExample() {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <BarChart data={cacheData} margin={{ top: 8, right: 8, left: -16, bottom: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" vertical={false} />
        <XAxis dataKey="type" tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} axisLine={false} tickLine={false} />
        <YAxis tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} axisLine={false} tickLine={false} />
        <Tooltip
          contentStyle={{
            backgroundColor: 'hsl(var(--card))',
            borderColor: 'hsl(var(--border))',
            borderRadius: 'var(--radius)',
            color: 'hsl(var(--card-foreground))',
          }}
        />
        <Bar dataKey="value" fill="hsl(var(--accent))" radius={[4, 4, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  )
}
