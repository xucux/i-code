import { useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Link } from '@tanstack/react-router'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useTheme } from '@/modules/theme/use-theme'
import type { AppLocale, AppTheme } from '@/core/types'
import { setLocale } from '@/modules/i18n/i18n'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Checkbox } from '@/components/ui/checkbox'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Slider } from '@/components/ui/slider'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Separator } from '@/components/ui/separator'
import { Progress } from '@/components/ui/progress'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet'
import {
  NavigationMenu,
  NavigationMenuContent,
  NavigationMenuItem,
  NavigationMenuLink,
  NavigationMenuList,
  NavigationMenuTrigger,
} from '@/components/ui/navigation-menu'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from '@/components/ui/hover-card'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  RadixScrollableTable,
  type RadixScrollableTableViewMode,
} from '@/components/ui/radix-scrollable-table'
import { ScrollPage } from '@/components/ui/scroll-page'
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarShortcut,
  MenubarTrigger,
} from '@/components/ui/menubar'
import {
  AreaChartExample,
  BarChartExample,
  LineChartExample,
} from '@/components/preview/chart-examples'
import { FileDropZone } from '@/components/preview/file-drop-zone'
import { CodeEditor } from '@/components/preview/code-editor'
// 自定义组件库新增组件：标题栏配置与托盘信息
import { TitleBarConfig } from '@/components/preview/title-bar-config'
import { TrayInfo } from '@/components/ui/tray-info'
import { TrayProviderSelector } from '@/components/ui/tray-provider-selector'
// 本次新增的 UI 组件库组件
import { Dropdown } from '@/components/ui/dropdown'
import { DropdownSearch } from '@/components/ui/dropdown-search'
import { LogViewer } from '@/components/ui/log-viewer'
import { LogPanel } from '@/components/ui/log-panel'
import { LogRollingConfig, type LogRollingConfigData } from '@/components/ui/log-rolling-config'
import { MiniFloatingPanel } from '@/components/ui/mini-floating-panel'
import { VirtualModelGraph, type VirtualModelTargetNode } from '@/components/ui/virtual-model-graph'
import { ModelMappingEditor, type ModelMappingItem } from '@/components/ui/model-mapping-editor'
// 本次新增的日期选择组件
import { DatePicker } from '@/components/ui/date-picker'
import { DateTimePicker } from '@/components/ui/date-time-picker'
import { DateRangePicker } from '@/components/ui/date-range-picker'
import { DateTimeRangePicker } from '@/components/ui/date-time-range-picker'
import type { DateRange } from 'react-day-picker'
import { LogTimeRangeFilter, type LogTimeRangeValue } from '@/modules/logger/ui/log-time-range-filter'
// 本次新增的业务组件演示
import { CliManagementDemo } from '@/components/preview/cli-management-demo'
import { WorkspaceDemo } from '@/components/preview/workspace-demo'

// Font Awesome 图标类名辅助函数，便于统一维护尺寸与样式
function iconClass(name: string, className?: string) {
  return cn('fa-solid', `fa-${name}`, className)
}

const requestRows = [
  { id: 'req-001', endpoint: '/v1/chat/completions', status: '200', latency: '120ms', time: '10:23:45' },
  { id: 'req-002', endpoint: '/v1/embeddings', status: '200', latency: '45ms', time: '10:24:12' },
  { id: 'req-003', endpoint: '/v1/models', status: '429', latency: '-', time: '10:25:01' },
  { id: 'req-004', endpoint: '/v1/chat/completions', status: '200', latency: '210ms', time: '10:26:33' },
]

// RadixScrollableTable 演示数据：列数较多，用于展示展开模式下的横向滚动
const radixTableRows = [
  { id: 'req-001', endpoint: '/v1/chat/completions', method: 'POST', status: '200', latency: '120ms', time: '10:23:45', region: 'ap-east-1', user: 'admin', version: 'v2' },
  { id: 'req-002', endpoint: '/v1/embeddings', method: 'POST', status: '200', latency: '45ms', time: '10:24:12', region: 'us-west-2', user: 'cli', version: 'v2' },
  { id: 'req-003', endpoint: '/v1/models', method: 'GET', status: '429', latency: '-', time: '10:25:01', region: 'eu-central-1', user: 'gateway', version: 'v1' },
  { id: 'req-004', endpoint: '/v1/chat/completions', method: 'POST', status: '200', latency: '210ms', time: '10:26:33', region: 'ap-northeast-1', user: 'admin', version: 'v2' },
  { id: 'req-005', endpoint: '/v1/completions', method: 'POST', status: '500', latency: '340ms', time: '10:27:18', region: 'us-east-1', user: 'internal', version: 'v1' },
  { id: 'req-006', endpoint: '/v1/chat/completions', method: 'POST', status: '200', latency: '98ms', time: '10:28:05', region: 'ap-southeast-1', user: 'cli', version: 'v2' },
  { id: 'req-007', endpoint: '/v1/images/generations', method: 'POST', status: '200', latency: '2.1s', time: '10:29:42', region: 'us-west-2', user: 'admin', version: 'v2' },
  { id: 'req-008', endpoint: '/v1/chat/completions', method: 'POST', status: '200', latency: '156ms', time: '10:30:11', region: 'eu-west-1', user: 'gateway', version: 'v2' },
]

// 下拉组件演示数据
const providerOptions = [
  { value: 'openai', label: 'OpenAI', icon: 'fa-solid fa-robot' },
  { value: 'anthropic', label: 'Anthropic', icon: 'fa-solid fa-sparkles' },
  { value: 'gemini', label: 'Google Gemini', icon: 'fa-solid fa-brain' },
  { value: 'deepseek', label: 'DeepSeek', icon: 'fa-solid fa-water' },
]

// 日志浏览组件演示数据
const sampleLogs = [
  { id: 'log-1', level: 'info' as const, timestamp: new Date().toISOString(), source: 'gateway', message: 'Gateway started on 127.0.0.1:3000' },
  { id: 'log-2', level: 'debug' as const, timestamp: new Date(Date.now() - 2000).toISOString(), source: 'balancer', message: 'Selected provider: OpenAI' },
  { id: 'log-3', level: 'warn' as const, timestamp: new Date(Date.now() - 5000).toISOString(), source: 'cache', message: 'Cache miss rate exceeded 30%' },
  { id: 'log-4', level: 'error' as const, timestamp: new Date(Date.now() - 8000).toISOString(), source: 'runtime', message: 'Provider Anthropic returned 429' },
]

// 迷你悬浮面板演示数据：模拟近 7 次模型消耗趋势
const miniChartData = [
  { label: '10:00', value: 1200 },
  { label: '10:05', value: 1850 },
  { label: '10:10', value: 1600 },
  { label: '10:15', value: 2400 },
  { label: '10:20', value: 2100 },
  { label: '10:25', value: 3200 },
  { label: '10:30', value: 2800 },
]

// 虚拟模型关系图演示数据：左侧父级虚拟模型，右侧子级供应商模型
const defaultModelMappings: ModelMappingItem[] = [
  { id: 'map-sonnet', role: 'Sonnet', displayName: 'claude-opus-4-8', actualModel: 'claude-opus-4-8[1M]', supports1M: true },
  { id: 'map-opus', role: 'Opus', displayName: 'claude-opus-4-8', actualModel: 'claude-opus-4-8[1M]', supports1M: true },
  { id: 'map-fable', role: 'Fable', displayName: 'claude-opus-4-8', actualModel: 'claude-opus-4-8[1M]', supports1M: true },
  { id: 'map-haiku', role: 'Haiku', displayName: 'claude-sonnet-4-5-20250929', actualModel: 'claude-sonnet-4-5-20250929', supports1M: false },
]

const virtualModelTargets: VirtualModelTargetNode[] = [
  {
    id: 'virtual-1',
    provider: '虚拟供应商',
    model: 'smart-failover-gpt',
    priority: 0,
    healthy: true,
    enabled: true,
  },
  {
    id: 'route-1',
    parentId: 'virtual-1',
    provider: 'OpenAI 渠道 A',
    model: 'gpt-4o',
    priority: 1,
    healthy: true,
    enabled: true,
    quotaPercent: 62,
    contextConfig: { temperature: 0.7, max_tokens: 4096 },
    features: { toolCalling: true, imageInput: true, streaming: true },
  },
  {
    id: 'route-2',
    parentId: 'virtual-1',
    provider: 'OpenAI 渠道 B',
    model: 'gpt-4o',
    priority: 2,
    healthy: true,
    enabled: true,
    quotaPercent: 88,
    contextConfig: '{"temperature":0.7,"top_p":0.95}',
    features: { toolCalling: true, imageInput: true, streaming: true },
  },
  {
    id: 'route-3',
    parentId: 'virtual-1',
    provider: 'NVIDIA',
    model: 'llama-3.1-405b',
    priority: 3,
    healthy: false,
    enabled: true,
    quotaPercent: 45,
    contextConfig: { temperature: 0.6, max_tokens: 2048 },
    features: { toolCalling: true, streaming: true },
  },
  {
    id: 'route-4',
    parentId: 'virtual-1',
    provider: 'DeepSeek',
    model: 'deepseek-chat',
    priority: 4,
    healthy: true,
    enabled: true,
    quotaPercent: 30,
    contextConfig: { temperature: 0.8 },
    features: { toolCalling: true, streaming: true },
  },
  {
    id: 'route-5',
    parentId: 'virtual-1',
    provider: '硅基流动',
    model: 'deepseek-ai/deepseek-v3',
    priority: 5,
    enabled: true,
    quotaPercent: 12,
    contextConfig: { temperature: 0.75, max_tokens: 8192 },
    features: { streaming: true },
  },
  {
    id: 'route-6',
    parentId: 'virtual-1',
    provider: 'Opus 4.8 中转',
    model: 'claude-3-opus',
    priority: 6,
    healthy: true,
    enabled: false,
    contextConfig: { temperature: 0.5, max_tokens: 4096 },
    features: { toolCalling: true, imageInput: true, streaming: true },
  },
]

const sampleYaml = `# AI Gateway Configuration
providers:
  - id: openai
    name: OpenAI
    baseUrl: https://api.openai.com/v1
    timeout: 30
models:
  - id: gpt-4o
    providerId: openai
    alias: gpt4
`

// 竖置菜单示例组件
function VerticalMenu() {
  const items = [
    { icon: 'house', label: 'Home', active: true },
    { icon: 'gear', label: 'Settings', active: false },
    { icon: 'credit-card', label: 'Billing', active: false },
    { icon: 'file-lines', label: 'Logs', active: false },
  ]

  return (
    <nav className="w-full rounded-md border p-1 md:w-56">
      <ul className="space-y-0.5">
        {items.map((item) => (
          <li key={item.label}>
            <button
              type="button"
              className={`flex w-full items-center gap-2 rounded-sm px-3 py-2 text-left text-xs transition-colors ${
                item.active
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-muted'
              }`}
            >
              <i className={iconClass(item.icon, 'size-4')} />
              {item.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  )
}

export default function PreviewPage() {
  const { t, i18n } = useTranslation('preview')
  const { theme, setTheme, toggleTheme } = useTheme()
  const [enabled, setEnabled] = useState(true)
  const [sliderValue, setSliderValue] = useState([50])
  const [radioValue, setRadioValue] = useState('comfortable')
  const [progress, setProgress] = useState(66)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [dialogValue, setDialogValue] = useState('')
  const [yamlValue, setYamlValue] = useState(sampleYaml)
  // 新增组件演示状态
  const [dropdownValue, setDropdownValue] = useState('openai')
  const [dropdownSearchValue, setDropdownSearchValue] = useState('gpt-4o')
  const [activeLogId, setActiveLogId] = useState<string | undefined>()
  const [selectedRouteId, setSelectedRouteId] = useState<string | undefined>('route-1')
  const [modelMappings, setModelMappings] = useState<ModelMappingItem[]>(defaultModelMappings)
  const [fallbackModel, setFallbackModel] = useState('claude-opus-4-8')
  const [availableModels, setAvailableModels] = useState<string[]>([])
  const [logRollingConfig, setLogRollingConfig] = useState<LogRollingConfigData>({
    enabled: true,
    maxFileSizeMb: 10,
    maxFileCount: 5,
    retentionDays: 7,
    bufferSize: 50,
  })
  const [radixTableViewMode, setRadixTableViewMode] = useState<RadixScrollableTableViewMode>('compact')

  // 日期选择组件演示状态
  const [dateValue, setDateValue] = useState<Date>()
  const [dateTimeValue, setDateTimeValue] = useState<Date>()
  const [dateRange, setDateRange] = useState<DateRange>()
  const [dateTimeRange, setDateTimeRange] = useState<DateRange>()
  const [logTimeRange, setLogTimeRange] = useState<LogTimeRangeValue>()

  // 根据时间范围筛选演示日志
  const filteredSampleLogs = useMemo(() => {
    if (!logTimeRange?.from && !logTimeRange?.to) return sampleLogs
    return sampleLogs.filter((log) => {
      if (logTimeRange.from && log.timestamp < logTimeRange.from) return false
      if (logTimeRange.to && log.timestamp > logTimeRange.to) return false
      return true
    })
  }, [logTimeRange])

  const handleLocaleChange = (value: string) => {
    const locale = value as AppLocale
    void i18n.changeLanguage(locale)
    setLocale(locale)
  }

  const handleThemeChange = (value: string) => {
    setTheme(value as AppTheme)
  }

  return (
    <TooltipProvider>
      <div className="bg-background text-foreground min-h-[calc(100vh-2.25rem)]">
        {/* 页面顶部工具栏：语言、主题切换与返回首页 */}
        <header className="border-b">
          <div className="mx-auto flex h-14 max-w-7xl items-center justify-between px-4">
            <div className="flex items-center gap-2.5">
              <div className="bg-primary text-primary-foreground flex size-7 items-center justify-center rounded-md text-sm font-bold">
                i
              </div>
              <h1 className="text-lg font-semibold tracking-tight">{t('title')}</h1>
            </div>
            <div className="flex items-center gap-3">
              <Select value={i18n.language} onValueChange={handleLocaleChange}>
                <SelectTrigger className="w-32 text-xs">
                  <i className={iconClass('globe', 'mr-1.5 size-3.5')} />
                  <SelectValue placeholder={t('localeSwitch')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="zh-CN">{t('locale.zhCN', { ns: 'locale' })}</SelectItem>
                  <SelectItem value="en">{t('locale.en', { ns: 'locale' })}</SelectItem>
                </SelectContent>
              </Select>

              <Select value={theme} onValueChange={handleThemeChange}>
                <SelectTrigger className="w-40 text-xs">
                  <i className={iconClass('palette', 'mr-1.5 size-3.5')} />
                  <SelectValue placeholder={t('themeSwitch')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="light">{t('theme.light', { ns: 'theme' })}</SelectItem>
                  <SelectItem value="dark">{t('theme.dark', { ns: 'theme' })}</SelectItem>
                  <SelectItem value="claude-light">{t('theme.claudeLight', { ns: 'theme' })}</SelectItem>
                  <SelectItem value="claude-dark">{t('theme.claudeDark', { ns: 'theme' })}</SelectItem>
                  <SelectItem value="deepseek-light">{t('theme.deepseekLight', { ns: 'theme' })}</SelectItem>
                  <SelectItem value="deepseek-dark">{t('theme.deepseekDark', { ns: 'theme' })}</SelectItem>
                </SelectContent>
              </Select>

              <Button variant="outline" size="icon" onClick={toggleTheme}>
                {theme.includes('dark') ? (
                  <i className={iconClass('moon', 'size-4')} />
                ) : (
                  <i className={iconClass('sun', 'size-4')} />
                )}
              </Button>

              <Button variant="outline" size="icon" asChild>
                <Link to="/">
                  <i className={iconClass('chevron-left', 'size-4')} />
                </Link>
              </Button>
            </div>
          </div>
        </header>

        {/* 组件分类标签页 */}
        <main className="mx-auto max-w-7xl px-4 py-6">
          <p className="text-muted-foreground mb-6 text-sm">{t('description')}</p>

          <Tabs defaultValue="buttons" className="w-full">
            <ScrollPage
              orientation="horizontal"
              variant="borderless"
              scrollbarThickness="thin"
              scrollbarVisible="auto"
              hideDelay={1500}
              className="mb-4"
            >
              <TabsList className="inline-flex text-[11px]">
                <TabsTrigger value="typography" className="shrink-0">{t('typography')}</TabsTrigger>
                <TabsTrigger value="buttons" className="shrink-0">{t('buttons')}</TabsTrigger>
                <TabsTrigger value="forms" className="shrink-0">{t('forms')}</TabsTrigger>
                <TabsTrigger value="overlays" className="shrink-0">{t('overlays')}</TabsTrigger>
                <TabsTrigger value="menus" className="shrink-0">{t('menus')}</TabsTrigger>
                <TabsTrigger value="data" className="shrink-0">{t('dataDisplay')}</TabsTrigger>
                <TabsTrigger value="charts" className="shrink-0">{t('charts')}</TabsTrigger>
                <TabsTrigger value="editor" className="shrink-0">{t('editor')}</TabsTrigger>
                <TabsTrigger value="feedback" className="shrink-0">{t('feedback')}</TabsTrigger>
                <TabsTrigger value="widgets" className="shrink-0">{t('widgets')}</TabsTrigger>
                <TabsTrigger value="titlebar" className="shrink-0">{t('titlebar')}</TabsTrigger>
                <TabsTrigger value="business" className="shrink-0">{t('business')}</TabsTrigger>
                <TabsTrigger value="scroll" className="shrink-0">{t('scroll')}</TabsTrigger>
                <TabsTrigger value="radixTable" className="shrink-0">{t('radixTable')}</TabsTrigger>
              </TabsList>
            </ScrollPage>

            {/* 字体排版 */}
            <TabsContent value="typography" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('typography')}</CardTitle>
                        <CardDescription className="text-xs">System font stack, type scale, and font weight guidelines.</CardDescription>
                      </CardHeader>
                        <CardContent className="space-y-6">
                          {/* 字号层级 */}
                          <div className="space-y-2">
                            <p className="text-sm font-medium">Type Scale</p>
                              <div className="space-y-2 rounded-md border p-3">
                                <div className="flex items-baseline justify-between">
                                  <span className="text-2xl font-bold">Display 24px</span>
                                    <span className="text-xs text-muted-foreground">text-2xl font-bold</span>
                                  </div>
                                    <div className="flex items-baseline justify-between">
                                      <span className="text-xl font-semibold">Heading 20px</span>
                                        <span className="text-xs text-muted-foreground">text-xl font-semibold</span>
                                      </div>
                                        <div className="flex items-baseline justify-between">
                                          <span className="text-lg font-semibold">Page Title 18px</span>
                                            <span className="text-xs text-muted-foreground">text-lg font-semibold</span>
                                          </div>
                                            <div className="flex items-baseline justify-between">
                                              <span className="text-base font-medium">Section Title 16px</span>
                                                <span className="text-xs text-muted-foreground">text-base font-medium</span>
                                              </div>
                                                <div className="flex items-baseline justify-between">
                                                  <span className="text-sm">Body 14px</span>
                                                    <span className="text-xs text-muted-foreground">text-sm</span>
                                                  </div>
                                                    <div className="flex items-baseline justify-between">
                                                      <span className="text-xs text-muted-foreground">Caption 12px</span>
                                                        <span className="text-xs text-muted-foreground">text-xs</span>
                                                      </div>
                                                        <div className="flex items-baseline justify-between">
                                                          <span className="text-[10px] text-muted-foreground">Tiny 10px</span>
                                                            <span className="text-xs text-muted-foreground">text-[10px]</span>
                                                          </div>
                                                        </div>
                                                      </div>

                                                        {/* 字重 */}
                                                        <div className="space-y-2">
                                                          <p className="text-sm font-medium">Font Weight</p>
                                                            <div className="grid grid-cols-2 gap-2 rounded-md border p-3 text-sm">
                                                              <span className="font-normal">font-normal 常规</span>
                                                                <span className="font-medium">font-medium 中等</span>
                                                                  <span className="font-semibold">font-semibold 半粗</span>
                                                                    <span className="font-bold">font-bold 粗体</span>
                                                                  </div>
                                                                </div>

                                                                  {/* 等宽数字 */}
                                                                  <div className="space-y-2">
                                                                    <p className="text-sm font-medium">Tabular Numbers</p>
                                                                      <div className="flex gap-4 rounded-md border p-3 text-sm">
                                                                        <span className="font-medium tabular-nums">1,234,567</span>
                                                                          <span className="font-medium tabular-nums">9,876,543</span>
                                                                            <span className="font-medium tabular-nums">0.00 KB</span>
                                                                          </div>
                                                                        </div>
                                                                      </CardContent>
                                                                    </Card>

                                                                  </div>
                                                                </ScrollPage>
                                                              </TabsContent>

            {/* 按钮 */}
            <TabsContent value="buttons" className="space-y-4">
              <Card>
                <CardHeader>
                  <CardTitle className="text-base">{t('buttons')}</CardTitle>
                  <CardDescription className="text-xs">Button variants, sizes, and states.</CardDescription>
                </CardHeader>
                <CardContent className="flex flex-wrap gap-3">
                  <Button>Default</Button>
                  <Button variant="secondary">Secondary</Button>
                  <Button variant="destructive">Destructive</Button>
                  <Button variant="outline">Outline</Button>
                  <Button variant="ghost">Ghost</Button>
                  <Button variant="link">Link</Button>
                </CardContent>
                <CardContent className="flex flex-wrap gap-3">
                  <Button size="sm">Small</Button>
                  <Button>Default</Button>
                  <Button size="lg">Large</Button>
                  <Button size="icon">
                    <i className={iconClass('gear', 'size-4')} />
                  </Button>
                </CardContent>
                <CardFooter className="flex gap-3">
                  <Button disabled>Disabled</Button>
                  <Button onClick={() => toast.success('Hello from sonner!')}>
                    <i className={iconClass('check', 'mr-1.5 size-4')} />
                    Trigger Toast
                  </Button>
                </CardFooter>
              </Card>
            </TabsContent>

            {/* 表单 */}
            <TabsContent value="forms" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('forms')}</CardTitle>
                        <CardDescription className="text-xs">Form controls and inputs.</CardDescription>
                      </CardHeader>
                        <CardContent className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-1.5">
                            <Label htmlFor="email" className="text-xs">Email</Label>
                              <Input id="email" placeholder="name@example.com" />
                          </div>
                            <div className="space-y-1.5">
                              <Label htmlFor="password" className="text-xs">Password</Label>
                                <Input id="password" type="password" placeholder="••••••••" />
                            </div>
                              <div className="space-y-1.5 md:col-span-2">
                                <Label htmlFor="search" className="text-xs">Search</Label>
                                  <Input
                                  id="search"
                                  placeholder="Search..."
                                  startIcon={<i className={iconClass('magnifying-glass', 'size-4')} />}
                                  />
                              </div>
                                <div className="space-y-1.5 md:col-span-2">
                                  <Label htmlFor="secret" className="text-xs">Secret</Label>
                                    <Input
                                    id="secret"
                                    type="password"
                                    placeholder="API key"
                                    startIcon={<i className={iconClass('envelope', 'size-4')} />}
                                    endIcon={<i className={iconClass('eye', 'size-4')} />}
                                    />
                                </div>
                                  <div className="space-y-1.5 md:col-span-2">
                                    <Label htmlFor="bio" className="text-xs">Bio</Label>
                                      <Textarea id="bio" placeholder="Tell us about yourself..." />
                                  </div>
                                    <div className="space-y-1.5">
                                      <Label className="text-xs">Role</Label>
                                        <Select defaultValue="admin">
                                          <SelectTrigger>
                                            <SelectValue placeholder="Select role" />
                                        </SelectTrigger>
                                          <SelectContent>
                                            <SelectItem value="admin">Admin</SelectItem>
                                              <SelectItem value="user">User</SelectItem>
                                                <SelectItem value="guest">Guest</SelectItem>
                                              </SelectContent>
                                            </Select>
                                          </div>
                                            <div className="flex items-center justify-between rounded-lg border p-3">
                                              <div className="space-y-0.5">
                                                <Label htmlFor="airplane" className="text-xs">Airplane Mode</Label>
                                                  <p className="text-muted-foreground text-xs">Disable all connections.</p>
                                                </div>
                                                  <Switch id="airplane" checked={enabled} onCheckedChange={setEnabled} />
                                              </div>
                                                <div className="flex items-center gap-2">
                                                  <Checkbox id="terms" />
                                                  <Label htmlFor="terms" className="text-xs font-normal">
                                                    Accept terms and conditions
                                                </Label>
                                              </div>
                                                <div className="space-y-2">
                                                  <Label className="text-xs">Density</Label>
                                                    <RadioGroup value={radioValue} onValueChange={setRadioValue}>
                                                      <div className="flex items-center gap-2">
                                                        <RadioGroupItem value="compact" id="compact" />
                                                        <Label htmlFor="compact" className="text-xs font-normal">Compact</Label>
                                                      </div>
                                                        <div className="flex items-center gap-2">
                                                          <RadioGroupItem value="comfortable" id="comfortable" />
                                                          <Label htmlFor="comfortable" className="text-xs font-normal">Comfortable</Label>
                                                        </div>
                                                      </RadioGroup>
                                                    </div>
                                                      <div className="space-y-2">
                                                        <Label className="text-xs">Volume: {sliderValue[0]}%</Label>
                                                          <Slider value={sliderValue} onValueChange={setSliderValue} max={100} step={1} />
                                                      </div>
                                                    </CardContent>
                                                  </Card>

                                                </div>
                                              </ScrollPage>
                                            </TabsContent>

            {/* 浮层 */}
            <TabsContent value="overlays" className="space-y-4">
              <Card>
                <CardHeader>
                  <CardTitle className="text-base">{t('overlays')}</CardTitle>
                  <CardDescription className="text-xs">Dialog, dropdown, popover, tooltip, hover card, sheet.</CardDescription>
                </CardHeader>
                <CardContent className="flex flex-wrap gap-3">
                  <Dialog>
                    <DialogTrigger asChild>
                      <Button variant="outline">Open Dialog</Button>
                    </DialogTrigger>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>Are you sure?</DialogTitle>
                        <DialogDescription>
                          This action cannot be undone. This will permanently delete your account.
                        </DialogDescription>
                      </DialogHeader>
                      <DialogFooter>
                        <Button variant="outline">Cancel</Button>
                        <Button variant="destructive">Delete</Button>
                      </DialogFooter>
                    </DialogContent>
                  </Dialog>

                  <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
                    <DialogTrigger asChild>
                      <Button variant="outline">Input Dialog</Button>
                    </DialogTrigger>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>Rename</DialogTitle>
                        <DialogDescription>
                          Enter a new name for this item.
                        </DialogDescription>
                      </DialogHeader>
                      <div className="grid gap-3 py-2">
                        <Label htmlFor="name" className="text-xs">Name</Label>
                        <Input
                          id="name"
                          value={dialogValue}
                          onChange={(e) => setDialogValue(e.target.value)}
                          placeholder="New name"
                        />
                      </div>
                      <DialogFooter>
                        <Button variant="outline" onClick={() => setDialogOpen(false)}>Cancel</Button>
                        <Button onClick={() => { toast.success(`Saved: ${dialogValue}`); setDialogOpen(false) }}>Save</Button>
                      </DialogFooter>
                    </DialogContent>
                  </Dialog>

                  <Sheet>
                    <SheetTrigger asChild>
                      <Button variant="outline">Open Drawer</Button>
                    </SheetTrigger>
                    <SheetContent>
                      <SheetHeader>
                        <SheetTitle>Drawer Title</SheetTitle>
                        <SheetDescription>
                          Side panel for navigation or detailed forms.
                        </SheetDescription>
                      </SheetHeader>
                      <div className="py-4">
                        <p className="text-xs text-muted-foreground">
                          Drawer content goes here. Use it for settings, details, or wizards.
                        </p>
                      </div>
                      <SheetFooter>
                        <Button>Confirm</Button>
                      </SheetFooter>
                    </SheetContent>
                  </Sheet>

                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="outline">Dropdown</Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent className="w-44">
                      <DropdownMenuLabel>My Account</DropdownMenuLabel>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem>Profile</DropdownMenuItem>
                      <DropdownMenuItem>Billing</DropdownMenuItem>
                      <DropdownMenuItem>Settings</DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem className="text-destructive">
                        <i className={iconClass('trash', 'mr-1.5 size-4')} />
                        Delete
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>

                  <Popover>
                    <PopoverTrigger asChild>
                      <Button variant="outline">Popover</Button>
                    </PopoverTrigger>
                    <PopoverContent className="w-72">
                      <div className="grid gap-3">
                        <div className="space-y-1">
                          <h4 className="font-medium leading-none text-sm">Dimensions</h4>
                          <p className="text-muted-foreground text-xs">Set the dimensions for the layer.</p>
                        </div>
                        <div className="grid gap-2">
                          <div className="grid grid-cols-3 items-center gap-3">
                            <Label htmlFor="width" className="text-xs">Width</Label>
                            <Input id="width" defaultValue="100%" className="col-span-2" />
                          </div>
                        </div>
                      </div>
                    </PopoverContent>
                  </Popover>

                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button variant="outline">Tooltip</Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      <p className="text-xs">Add to library</p>
                    </TooltipContent>
                  </Tooltip>

                  <HoverCard>
                    <HoverCardTrigger asChild>
                      <Button variant="link">Hover Card</Button>
                    </HoverCardTrigger>
                    <HoverCardContent className="w-56">
                      <div className="flex justify-between space-x-3">
                        <Avatar className="size-8">
                          <AvatarImage src="https://github.com/shadcn.png" />
                          <AvatarFallback>SC</AvatarFallback>
                        </Avatar>
                        <div className="space-y-0.5">
                          <h4 className="text-xs font-semibold">@shadcn</h4>
                          <p className="text-xs">Designer & Engineer.</p>
                        </div>
                      </div>
                    </HoverCardContent>
                  </HoverCard>
                </CardContent>
              </Card>
            </TabsContent>

            {/* 菜单 */}
            <TabsContent value="menus" className="space-y-4">
              <Card>
                <CardHeader>
                  <CardTitle className="text-base">{t('menus')}</CardTitle>
                  <CardDescription className="text-xs">Horizontal and vertical navigation menus.</CardDescription>
                </CardHeader>
                <CardContent className="grid gap-6">
                  <div className="rounded-md border p-2">
                    <NavigationMenu>
                      <NavigationMenuList>
                        <NavigationMenuItem>
                          <NavigationMenuTrigger className="text-xs">Getting Started</NavigationMenuTrigger>
                          <NavigationMenuContent>
                            <ul className="grid gap-2 p-3 md:w-48">
                              <li>
                                <NavigationMenuLink asChild>
                                  <a className="block rounded-md p-2 text-xs hover:bg-muted" href="#">Introduction</a>
                                </NavigationMenuLink>
                              </li>
                              <li>
                                <NavigationMenuLink asChild>
                                  <a className="block rounded-md p-2 text-xs hover:bg-muted" href="#">Installation</a>
                                </NavigationMenuLink>
                              </li>
                            </ul>
                          </NavigationMenuContent>
                        </NavigationMenuItem>
                        <NavigationMenuItem>
                          <NavigationMenuLink className="text-xs">
                            <a href="#" className="px-3 py-2">Documentation</a>
                          </NavigationMenuLink>
                        </NavigationMenuItem>
                        <NavigationMenuItem>
                          <NavigationMenuLink className="text-xs">
                            <a href="#" className="px-3 py-2">Settings</a>
                          </NavigationMenuLink>
                        </NavigationMenuItem>
                      </NavigationMenuList>
                    </NavigationMenu>
                  </div>
                  <VerticalMenu />
                </CardContent>
              </Card>
            </TabsContent>

            {/* 数据展示 */}
            <TabsContent value="data" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('dataDisplay')}</CardTitle>
                        <CardDescription className="text-xs">Table, badge, avatar, separator, accordion, menubar, scroll area.</CardDescription>
                      </CardHeader>
                        <CardContent className="grid gap-4">
                          <Table>
                            <TableCaption className="text-xs">Recent API request records.</TableCaption>
                              <TableHeader>
                                <TableRow>
                                  <TableHead className="w-28 text-xs">ID</TableHead>
                                    <TableHead className="text-xs">Endpoint</TableHead>
                                      <TableHead className="w-20 text-xs">Status</TableHead>
                                        <TableHead className="w-20 text-xs">Latency</TableHead>
                                          <TableHead className="w-24 text-xs">Time</TableHead>
                                        </TableRow>
                                      </TableHeader>
                                        <TableBody>
                                          {requestRows.map((row) => (
                                          <TableRow key={row.id}>
                                            <TableCell className="font-medium text-xs">{row.id}</TableCell>
                                              <TableCell className="text-xs">{row.endpoint}</TableCell>
                                                <TableCell>
                                                  <Badge variant={row.status.startsWith('2') ? 'default' : 'destructive'} className="text-xs">
                                                    {row.status}
                                                </Badge>
                                              </TableCell>
                                                <TableCell className="text-xs">{row.latency}</TableCell>
                                                  <TableCell className="text-xs">{row.time}</TableCell>
                                                </TableRow>
                                                  ))}
                                              </TableBody>
                                            </Table>

                                              <div className="flex flex-wrap items-center gap-2">
                                                <Badge className="text-xs">Default</Badge>
                                                  <Badge variant="secondary" className="text-xs">Secondary</Badge>
                                                    <Badge variant="outline" className="text-xs">Outline</Badge>
                                                      <Badge variant="destructive" className="text-xs">Destructive</Badge>
                                                    </div>
                                                      <div className="flex flex-wrap items-center gap-2">
                                                        <Avatar className="size-8">
                                                          <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
                                                          <AvatarFallback>CN</AvatarFallback>
                                                      </Avatar>
                                                        <Avatar className="size-8">
                                                          <AvatarFallback>JD</AvatarFallback>
                                                      </Avatar>
                                                    </div>
                                                      <div>
                                                        <Menubar>
                                                          <MenubarMenu>
                                                            <MenubarTrigger className="text-xs">File</MenubarTrigger>
                                                              <MenubarContent>
                                                                <MenubarItem className="text-xs">
                                                                  New Tab <MenubarShortcut>⌘T</MenubarShortcut>
                                                              </MenubarItem>
                                                                <MenubarItem className="text-xs">New Window</MenubarItem>
                                                                  <MenubarSeparator />
                                                                  <MenubarItem className="text-xs">Share</MenubarItem>
                                                                </MenubarContent>
                                                              </MenubarMenu>
                                                                <MenubarMenu>
                                                                  <MenubarTrigger className="text-xs">Edit</MenubarTrigger>
                                                                    <MenubarContent>
                                                                      <MenubarItem className="text-xs">Undo</MenubarItem>
                                                                        <MenubarItem className="text-xs">Redo</MenubarItem>
                                                                      </MenubarContent>
                                                                    </MenubarMenu>
                                                                  </Menubar>
                                                                </div>
                                                                  <div>
                                                                    <Separator className="my-2" />
                                                                    <Accordion type="single" collapsible className="w-full">
                                                                      <AccordionItem value="item-1">
                                                                        <AccordionTrigger className="text-sm">Is it accessible?</AccordionTrigger>
                                                                          <AccordionContent className="text-xs">Yes. It adheres to the WAI-ARIA design pattern.</AccordionContent>
                                                                        </AccordionItem>
                                                                          <AccordionItem value="item-2">
                                                                            <AccordionTrigger className="text-sm">Is it styled?</AccordionTrigger>
                                                                              <AccordionContent className="text-xs">Yes. It comes with default styles that match the other components.</AccordionContent>
                                                                            </AccordionItem>
                                                                          </Accordion>
                                                                        </div>
                                                                          <ScrollArea className="h-28 rounded-md border p-3">
                                                                            <p className="text-xs leading-relaxed">
                                                                              Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et
                                                                              dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip
                                                                              ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu
                                                                              fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia
                                                                              deserunt mollit anim id est laborum.
                                                                          </p>
                                                                        </ScrollArea>
                                                                      </CardContent>
                                                                    </Card>

                                                                  </div>
                                                                </ScrollPage>
                                                              </TabsContent>

            {/* 图表 */}
            <TabsContent value="charts" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <div className="grid gap-4 md:grid-cols-2">
                    <Card>
                      <CardHeader>
                        <CardTitle className="text-base">{t('chart.request')}</CardTitle>
                          <CardDescription className="text-xs">{t('chart.requestDesc')}</CardDescription>
                        </CardHeader>
                          <CardContent>
                            <AreaChartExample />
                        </CardContent>
                      </Card>
                        <Card>
                          <CardHeader>
                            <CardTitle className="text-base">{t('chart.token')}</CardTitle>
                              <CardDescription className="text-xs">{t('chart.tokenDesc')}</CardDescription>
                            </CardHeader>
                              <CardContent>
                                <LineChartExample />
                            </CardContent>
                          </Card>
                            <Card className="md:col-span-2">
                              <CardHeader>
                                <CardTitle className="text-base">{t('chart.cache')}</CardTitle>
                                  <CardDescription className="text-xs">{t('chart.cacheDesc')}</CardDescription>
                                </CardHeader>
                                  <CardContent>
                                    <BarChartExample />
                                </CardContent>
                              </Card>
                            </div>

                          </div>
                        </ScrollPage>
                      </TabsContent>

            {/* 编辑器 */}
            <TabsContent value="editor" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('editor')}</CardTitle>
                        <CardDescription className="text-xs">{t('editorDesc')}</CardDescription>
                      </CardHeader>
                        <CardContent className="space-y-4">
                          <CodeEditor value={yamlValue} onChange={setYamlValue} language="yaml" minHeight="240px" />
                          <FileDropZone accept=".yaml,.yml,.json,.conf,.ini,.toml" maxFiles={3} />
                      </CardContent>
                    </Card>

                  </div>
                </ScrollPage>
              </TabsContent>

            {/* 标题栏与托盘 */}
            <TabsContent value="titlebar" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('titlebar')}</CardTitle>
                        <CardDescription className="text-xs">Title bar info config and tray preview.</CardDescription>
                      </CardHeader>
                        <CardContent className="grid gap-6 md:grid-cols-3">
                          {/* 标题栏信息配置演示：含内存占用开关 */}
                          <div className="md:col-span-2">
                            <TitleBarConfig />
                        </div>
                          {/* 托盘信息展示演示 */}
                          <div className="space-y-4">
                            <div className="space-y-2">
                              <p className="text-sm font-medium">托盘信息预览</p>
                                <TrayInfo
                                provider="OpenAI"
                                model="gpt-4o"
                                quota={{ used: 12846, total: 50000, unit: 'tokens' }}
                                />
                            </div>
                              <div className="space-y-2">
                                <p className="text-sm font-medium">托盘供应商选择</p>
                                  <TrayProviderSelector
                                  providers={[
                                  { id: 'openai', name: 'OpenAI' },
                                  { id: 'anthropic', name: 'Anthropic' },
                                  { id: 'gemini', name: 'Google Gemini' },
                                  { id: 'deepseek', name: 'DeepSeek' },
                                  ]}
                                  />
                              </div>
                            </div>
                          </CardContent>
                        </Card>

                      </div>
                    </ScrollPage>
                  </TabsContent>

            {/* 反馈 */}
            <TabsContent value="feedback" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('feedback')}</CardTitle>
                        <CardDescription className="text-xs">Alert, progress, and toast feedback.</CardDescription>
                      </CardHeader>
                        <CardContent className="space-y-4">
                          <Alert className="py-3">
                            <i className={iconClass('circle-info', 'size-4')} />
                            <AlertTitle className="text-sm">Heads up!</AlertTitle>
                              <AlertDescription className="text-xs">You can add components to your app using the cli.</AlertDescription>
                            </Alert>
                              <Alert variant="destructive" className="py-3">
                                <i className={iconClass('terminal', 'size-4')} />
                                <AlertTitle className="text-sm">Error</AlertTitle>
                                  <AlertDescription className="text-xs">Your session has expired. Please log in again.</AlertDescription>
                                </Alert>
                                  <div className="space-y-1.5">
                                    <div className="flex justify-between text-xs">
                                      <span>Progress</span>
                                      <span>{progress}%</span>
                                  </div>
                                    <Progress value={progress} />
                                    <div className="flex gap-2">
                                      <Button variant="outline" size="sm" onClick={() => setProgress((v) => Math.max(0, v - 10))}>
                                        Decrease
                                    </Button>
                                      <Button variant="outline" size="sm" onClick={() => setProgress((v) => Math.min(100, v + 10))}>
                                        Increase
                                    </Button>
                                  </div>
                                </div>
                                  <div className="flex flex-wrap gap-2">
                                    <Button variant="outline" onClick={() => toast('Default toast')}>
                                      Toast
                                  </Button>
                                    <Button variant="outline" onClick={() => toast.success('Success toast')}>
                                      Success
                                  </Button>
                                    <Button variant="outline" onClick={() => toast.error('Error toast')}>
                                      Error
                                  </Button>
                                    <Button variant="outline" onClick={() => toast.promise(new Promise((r) => setTimeout(r, 2000)), {
                                      loading: 'Loading...',
                                      success: 'Loaded!',
                                      error: 'Failed!',
                                      })}>
                                      Promise
                                  </Button>
                                </div>
                              </CardContent>
                            </Card>

                          </div>
                        </ScrollPage>
                      </TabsContent>

            {/* 业务组件展示：CLI 管理与工作区 */}
            <TabsContent value="business" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('business')}</CardTitle>
                        <CardDescription className="text-xs">{t('businessDesc')}</CardDescription>
                      </CardHeader>
                        <CardContent className="grid gap-6 lg:grid-cols-2">
                          <div className="space-y-2">
                            <p className="text-sm font-medium">CLI 管理</p>
                              <CliManagementDemo />
                          </div>
                            <div className="space-y-2">
                              <p className="text-sm font-medium">工作区</p>
                                <WorkspaceDemo />
                            </div>
                          </CardContent>
                        </Card>

                      </div>
                    </ScrollPage>
                  </TabsContent>

            {/* 新增组件展示：下拉、下拉搜索、日志浏览、迷你悬浮面板 */}
            <TabsContent value="widgets" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('widgets')}</CardTitle>
                        <CardDescription className="text-xs">Dropdown, dropdown search, date pickers, log viewer, and mini floating panel.</CardDescription>
                      </CardHeader>
                        <CardContent className="grid gap-6 lg:grid-cols-2">
                          {/* 下拉组件演示 */}
                          <div className="space-y-2">
                            <p className="text-sm font-medium">Dropdown</p>
                              <Dropdown
                              options={providerOptions}
                              value={dropdownValue}
                              onChange={setDropdownValue}
                              placeholder="选择供应商"
                              />
                          </div>

                            {/* 下拉搜索组件演示 */}
                            <div className="space-y-2">
                              <p className="text-sm font-medium">Dropdown Search</p>
                                <DropdownSearch
                                options={[
                                { value: 'gpt-4o', label: 'GPT-4o', icon: 'fa-solid fa-robot' },
                                { value: 'gpt-4o-mini', label: 'GPT-4o mini', icon: 'fa-solid fa-robot' },
                                { value: 'claude-3-5-sonnet', label: 'Claude 3.5 Sonnet', icon: 'fa-solid fa-sparkles' },
                                { value: 'claude-3-opus', label: 'Claude 3 Opus', icon: 'fa-solid fa-sparkles' },
                                { value: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro', icon: 'fa-solid fa-brain' },
                                { value: 'deepseek-chat', label: 'DeepSeek Chat', icon: 'fa-solid fa-water' },
                                ]}
                                value={dropdownSearchValue}
                                onChange={setDropdownSearchValue}
                                placeholder="选择模型"
                                searchPlaceholder="搜索模型..."
                                />
                            </div>

                              {/* 日志列表组件演示 */}
                              <div className="space-y-2 lg:col-span-2">
                                <div className="flex items-center justify-between">
                                  <p className="text-sm font-medium">Log Viewer</p>
                                  <span className="text-[10px] text-muted-foreground">
                                    共 {filteredSampleLogs.length} 条
                                  </span>
                                </div>
                                  <LogViewer
                                  logs={filteredSampleLogs}
                                  activeId={activeLogId}
                                  onRowClick={(entry) => setActiveLogId(entry.id)}
                                  className="h-56"
                                  />
                              </div>

                                {/* 基于编辑器的日志面板演示：仅展示缓冲队列数据 */}
                                <div className="space-y-2 lg:col-span-2">
                                  <p className="text-sm font-medium">Log Panel（基于编辑器）</p>
                                    <LogPanel
                                    logs={sampleLogs}
                                    bufferSize={logRollingConfig.bufferSize}
                                    />
                                </div>

                                  {/* 日志滚动记录配置 */}
                                  <div className="space-y-2 lg:col-span-2">
                                    <LogRollingConfig
                                    value={logRollingConfig}
                                    onChange={setLogRollingConfig}
                                    />
                                </div>

                                  {/* 日期选择器组件演示 */}
                                  <div className="space-y-2">
                                    <p className="text-sm font-medium">Date Picker</p>
                                    <DatePicker value={dateValue} onChange={setDateValue} />
                                  </div>

                                  <div className="space-y-2">
                                    <p className="text-sm font-medium">DateTime Picker</p>
                                    <DateTimePicker value={dateTimeValue} onChange={setDateTimeValue} />
                                  </div>

                                  <div className="space-y-2">
                                    <p className="text-sm font-medium">Date Range Picker</p>
                                    <DateRangePicker value={dateRange} onChange={setDateRange} />
                                  </div>

                                  <div className="space-y-2">
                                    <p className="text-sm font-medium">DateTime Range Picker</p>
                                    <DateTimeRangePicker value={dateTimeRange} onChange={setDateTimeRange} />
                                  </div>

                                  {/* 日志时间范围筛选器：使用 DateTimeRangePicker */}
                                  <div className="space-y-2 lg:col-span-2">
                                    <p className="text-sm font-medium">Log Time Range Filter</p>
                                    <LogTimeRangeFilter value={logTimeRange} onChange={setLogTimeRange} />
                                    <p className="text-[10px] text-muted-foreground">
                                      value: {logTimeRange ? `${logTimeRange.from ?? '-'} ~ ${logTimeRange.to ?? '-'}` : 'undefined'}
                                    </p>
                                  </div>

                                  {/* 迷你悬浮面板：图表模式 */}
                                  <div className="space-y-2">
                                    <p className="text-sm font-medium">Mini Floating Panel（面积图模式）</p>
                                      <MiniFloatingPanel
                                      provider="OpenAI"
                                      currentModel="gpt-4o"
                                      totalQuota={100000}
                                      usedQuota={45230}
                                      modelConsumption={18500}
                                      gatewayUrl="127.0.0.1:3000"
                                      gatewayStatus="running"
                                      chartData={miniChartData}
                                      />
                                  </div>

                                    {/* 迷你悬浮面板：数字模式 */}
                                    <div className="space-y-2">
                                      <p className="text-sm font-medium">Mini Floating Panel（数字模式）</p>
                                        <MiniFloatingPanel
                                        provider="DeepSeek"
                                        currentModel="deepseek-chat"
                                        totalQuota={50000}
                                        usedQuota={12800}
                                        modelConsumption={3200}
                                        gatewayUrl="127.0.0.1:3000"
                                        gatewayStatus="idle"
                                        />
                                    </div>

                                      {/* 虚拟模型关系图演示 */}
                                      <div className="space-y-2 lg:col-span-2">
                                        <p className="text-sm font-medium">Virtual Model Graph</p>
                                          <VirtualModelGraph
                                          virtualModel="smart-failover-gpt"
                                          targets={virtualModelTargets}
                                          selectedId={selectedRouteId}
                                          onSelect={setSelectedRouteId}
                                          />
                                      </div>

                                        {/* 模型映射编辑器演示 */}
                                        <div className="space-y-2 lg:col-span-2">
                                          <p className="text-sm font-medium">Model Mapping Editor</p>
                                            <ModelMappingEditor
                                            mappings={modelMappings}
                                            fallbackModel={fallbackModel}
                                            availableModels={availableModels}
                                            onMappingsChange={setModelMappings}
                                            onFallbackChange={setFallbackModel}
                                            onAutoSetup={() => toast.success('一键设置（演示）')}
                                            onFetchModels={(provider) => {
                                            // 演示：模拟获取模型列表，有数据后右侧出现下拉图标
                                            setAvailableModels([
                                            'claude-opus-4-8',
                                            'claude-opus-4-8[1M]',
                                            'claude-sonnet-4-5-20250929',
                                            'claude-sonnet-4-5-20250929[1M]',
                                            'claude-haiku-4-2-20250514',
                                            ])
                                            toast.success(`获取模型列表: ${provider}`)
                                            }}
                                            />
                                        </div>
                                      </CardContent>
                                    </Card>

                                  </div>
                                </ScrollPage>
                              </TabsContent>

            {/* 滚动页面组件展示 */}
            <TabsContent value="scroll" className="h-[60vh]">
              <ScrollPage className="h-full rounded-md border">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <CardTitle className="text-base">{t('scroll')}</CardTitle>
                      <CardDescription className="text-xs">ScrollPage variants: borderless, auto-hide, thickness, length.</CardDescription>
                    </CardHeader>
                    <CardContent className="grid gap-4 md:grid-cols-2">
                      <div className="space-y-2">
                        <p className="text-sm font-medium">Default</p>
                        <ScrollPage className="h-32 rounded-md border">
                          <div className="space-y-2 p-3">
                            {Array.from({ length: 20 }).map((_, i) => (
                              <div key={i} className="text-xs text-muted-foreground">Line {i + 1}</div>
                            ))}
                          </div>
                        </ScrollPage>
                      </div>

                      <div className="space-y-2">
                        <p className="text-sm font-medium">Borderless + Auto-hide</p>
                        <ScrollPage variant="borderless" scrollbarVisible="auto" className="h-32">
                          <div className="space-y-2 p-3">
                            {Array.from({ length: 20 }).map((_, i) => (
                              <div key={i} className="text-xs text-muted-foreground">Line {i + 1}</div>
                            ))}
                          </div>
                        </ScrollPage>
                      </div>

                      <div className="space-y-2">
                        <p className="text-sm font-medium">Thick Scrollbar</p>
                        <ScrollPage scrollbarThickness="thick" className="h-32 rounded-md border">
                          <div className="space-y-2 p-3">
                            {Array.from({ length: 20 }).map((_, i) => (
                              <div key={i} className="text-xs text-muted-foreground">Line {i + 1}</div>
                            ))}
                          </div>
                        </ScrollPage>
                      </div>

                      <div className="space-y-2">
                        <p className="text-sm font-medium">Thin + Short Thumb</p>
                        <ScrollPage scrollbarThickness="thin" scrollbarLength="min-h-[1rem]" className="h-32 rounded-md border">
                          <div className="space-y-2 p-3">
                            {Array.from({ length: 20 }).map((_, i) => (
                              <div key={i} className="text-xs text-muted-foreground">Line {i + 1}</div>
                            ))}
                          </div>
                        </ScrollPage>
                      </div>
                    </CardContent>
                  </Card>
                </div>
              </ScrollPage>
            </TabsContent>

            {/* RadixScrollableTable 组件演示 */}
            <TabsContent value="radixTable" className="h-[60vh]">
              {/* <ScrollPage className="h-full rounded-md border"> */}
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader>
                      <div className="flex items-center justify-between">
                        <div>
                          <CardTitle className="text-base">{t('radixTable')}</CardTitle>
                          <CardDescription className="text-xs">{t('radixTableDesc')}</CardDescription>
                        </div>
                        <ToggleGroup
                          type="single"
                          value={radixTableViewMode}
                          onValueChange={(v) => v && setRadixTableViewMode(v as RadixScrollableTableViewMode)}
                          variant="outline"
                          size="sm"
                        >
                          <ToggleGroupItem value="compact" className="h-7 px-2 text-xs">
                            <i className={iconClass('table-cells', 'mr-1.5 size-3')} />
                            {t('compact')}
                          </ToggleGroupItem>
                          <ToggleGroupItem value="expanded" className="h-7 px-2 text-xs">
                            <i className={iconClass('table-columns', 'mr-1.5 size-3')} />
                            {t('expanded')}
                          </ToggleGroupItem>
                        </ToggleGroup>
                      </div>
                    </CardHeader>
                    <CardContent>
                      <RadixScrollableTable viewMode={radixTableViewMode} style={{ height: 320 }}>
                        <TableHeader className="sticky top-0 z-10 bg-muted/50">
                          <TableRow>
                            <TableHead className="w-20 text-xs">ID</TableHead>
                            <TableHead className="min-w-[180px] text-xs">Endpoint</TableHead>
                            <TableHead className="w-20 text-xs">Method</TableHead>
                            <TableHead className="w-20 text-xs">Status</TableHead>
                            <TableHead className="w-24 text-xs">Latency</TableHead>
                            <TableHead className="w-24 text-xs">Time</TableHead>
                            <TableHead className="w-28 text-xs">Region</TableHead>
                            <TableHead className="w-20 text-xs">User</TableHead>
                            <TableHead className="w-16 text-xs">Version</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {radixTableRows.map((row) => (
                            <TableRow key={row.id}>
                              <TableCell className="font-medium text-xs">{row.id}</TableCell>
                              <TableCell className="font-mono text-[10px]">{row.endpoint}</TableCell>
                              <TableCell className="text-xs">{row.method}</TableCell>
                              <TableCell>
                                <Badge
                                  variant={row.status.startsWith('2') ? 'default' : row.status.startsWith('4') ? 'secondary' : 'destructive'}
                                  className="text-[10px]"
                                >
                                  {row.status}
                                </Badge>
                              </TableCell>
                              <TableCell className="text-xs tabular-nums">{row.latency}</TableCell>
                              <TableCell className="text-xs tabular-nums">{row.time}</TableCell>
                              <TableCell className="text-xs">{row.region}</TableCell>
                              <TableCell className="text-xs">{row.user}</TableCell>
                              <TableCell className="text-xs">{row.version}</TableCell>
                            </TableRow>
                          ))}
                        </TableBody>
                      </RadixScrollableTable>
                    </CardContent>
                  </Card>
                </div>
              {/* </ScrollPage> */}
            </TabsContent>
          </Tabs>
        </main>
      </div>
    </TooltipProvider>
  )
}
