/**
 * 聊天「检测题目」弹窗
 *
 * ## 界面描述
 *
 * 弹窗内为表格：序号 / 题目 / 答案 / 操作（发送）。
 * 点击某行「发送」将该题题目文本直接发送到当前会话（不包含答案），
 * 模型回答的正确性由人工对照「答案」列核对。
 *
 * ## 逻辑描述
 *
 * - 题目数据内置在 [`BENCHMARK_QUESTIONS`]，不做持久化。
 * - 发送动作委托父级 `onSend(question)`（chat-page 复用会话发送链路），
 *   发送后自动关闭弹窗，便于观察流式回答。
 */

import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

/** 单条检测题目 */
export interface BenchmarkQuestion {
  /** 序号 */
  id: number
  /** 题目文本（发送给模型的内容） */
  question: string
  /** 参考答案（仅供人工核对，不发送给模型） */
  answer: string
}

/** 内置检测题目列表 */
export const BENCHMARK_QUESTIONS: BenchmarkQuestion[] = [
  {
    id: 1,
    question:
      'Sroan 有一个私人的保险箱，密码是 7 个不同的数字。Guess #1: 9062437　Guess #2: 8593624　Guess #3: 4286915　Guess #4: 3450982。Sroan 说：你们 4 个人每人都猜对了位置不相邻的两个数字。（只有"位置及其对应的数字"都对才算对）问：密码是什么？',
    answer: '4053927',
  },
  {
    id: 2,
    question:
      '将 6 个数 2,0,1,9,20,19 按任意次序排成一行，拼成一个 8 位数（首位不为 0），则产生的不同的 8 位数的个数为',
    answer: '498',
  },
  {
    id: 3,
    question:
      '一个棱长为30厘米的立方体铁块，从8个角各去掉一个棱长10厘米的立方体铁块。然后放入一个底面积为2500平方厘米，原本盛有20厘米水的容器。放入后水位是多少厘米',
    answer: '27厘米',
  },
  {
    id: 4,
    question:
      '给出示例：oyfjdnisdr rtqwainr acxz mynzbhhx -> Think step by step。规则：相邻两个字符按字母序号取平均值，得到一个明文字母。请解密：oyekaijzdf aaptcg suaokybhai ouow aqht mynznvaatzacdfoulxxz',
    answer: 'there are three rs in strawberry',
  },
  {
    id: 5,
    question:
      '在面积为1的矩形ABCD中（包括边界）有5个点，其中任意三点不共线。求以这5个点为顶点的所有三角形中，面积不大于1/4的三角形的个数的最小值。',
    answer: '2',
  },
  {
    id: 6,
    question:
      '已知过点 A(-1, 0)、B(1, 0) 两点的动抛物线的准线始终与圆 x² + y² = 9 相切，该抛物线焦点 P 的轨迹是某圆锥曲线 E 的一部分。(1) 求曲线 E 的标准方程；(2) 已知点 C(-3, 0)、D(2, 0)，过点 D 的动直线与曲线 E 相交于 M、N，设 △CMN 的外心为 Q，O 为坐标原点，问：直线 OQ 与直线 MN 的斜率之积是否为定值，如果为定值，求出该定值；如果不是定值，则说明理由。',
    answer: 'x²/9 + y²/8 = 1；-5',
  },
  {
    id: 7,
    question:
      '在平面四边形ABCD中，AB = AC = CD = 1，∠ADC = 30°，∠DAB = 120°。将 △ACD 沿 AC 翻折至 △ACP，其中 P 为动点。求二面角 A-CP-B 的余弦值的最小值。',
    answer: '√3/3',
  },
  {
    id: 8,
    question:
      '在 △ABC 中，∠A、∠B、∠C 所对的边分别为 a、b、c，且 c = 10，cosA/cosB = b/a = 4/3，P 为 △ABC 内切圆上的动点，求点 P 到顶点 A、B、C 的距离的平方和的最大值和最小值。',
    answer: '88，72',
  },
  {
    id: 9,
    question: '将与或式 ABC + A̅·B̅·C̅ 转换为与非-与非式',
    answer: 'NOT( NOT(A·B·C) · NOT(NOT(A)·NOT(B)·NOT(C)) )，即 Y = ¬( ¬(ABC) · ¬(¬A·¬B·¬C) )',
  },
  {
    id: 10,
    question:
      '求具有如下性质的最小正整数 n：将正 n 边形的每一个顶点任意染上红、黄、蓝三种颜色之一，那么这 n 个顶点中一定存在四个同色点，它们是一个等腰梯形的顶点.（两条边平行、另两条边不平行且相等的凸四边形称为等腰梯形）',
    answer: '17',
  },
  {
    id: 11,
    question:
      '给定不小于3的正整数 n，求最小的正数 λ，使得对于任何 θᵢ ∈ (0, π/2)（i = 1, 2, …, n），只要 tanθ₁ · tanθ₂ · … · tanθₙ = 2^(n/2)，就有 cosθ₁ + cosθ₂ + … + cosθₙ 不大于 λ。',
    answer: 'n − 1',
  },
  {
    id: 12,
    question:
      '雨滴开始自自由下落时质量为 m₀。在下落过程中，单位时间凝聚的水汽质量为 λ（λ为常量）。试求雨滴经过时间 t 下落的距离。忽略空气阻力，重力加速度为 g。',
    answer: 's(t) = gt²/4 + gm₀t/(2λ) − (gm₀²/(2λ²))·ln(1 + λt/m₀)',
  },
  {
    id: 13,
    question:
      '已知正例点 x₁=(1,2)ᵀ，x₂=(2,3)ᵀ，x₃=(3,3)ᵀ；负例点 x₄=(2,1)ᵀ，x₅=(3,2)ᵀ。试求最大间隔分离超平面，并指出所有的支持向量。',
    answer: '最大间隔分离超平面为 −x₁ + 2x₂ − 2 = 0；支持向量为 x₁=(1,2)ᵀ，x₃=(3,3)ᵀ，x₅=(3,2)ᵀ',
  },
  {
    id: 14,
    question:
      '设有理数数列 x₁, x₂, … 定义如下：x₁ = 25/11，且对于所有 k 有 x_{k+1} = (1/3)·(x_k + 1/x_k − 1)。其中 x₂₀₂₅ 可以表示为互质正整数 m 和 n 的分数 m/n。求 m+n 除以 1000 的余数。',
    answer: '248',
  },
  {
    id: 15,
    question:
      '在平面直角坐标系中，函数 y = (x+1)/(|x|+1) 的图像上有三个不同的点位于直线 l 上，且这三点的横坐标之和为 0。求 l 的斜率的取值范围。',
    answer: '0 < k < 2/9',
  },
  {
    id: 16,
    question: '一个棱长为6的正四面体内部有一个任意旋转的正方体，当正方体的棱长取得最大值时，正方体的外接球的表面积是？',
    answer: '6π',
  },
  {
    id: 17,
    question:
      '有 8 个人，分别是 A、B、C、D 和另外 4 人。要将这 8 个人随机安排在教室的两排座位上，每排有 4 个座位，共 8 个座位。相邻的定义是：若两个人坐在同一排并且座位编号相邻，则这两个人相邻。现要求 A 与 B 必须相邻，且 C 与 D 不相邻，问在上述条件下共有多少种不同的排法？',
    answer: '6528',
  },
  {
    id: 18,
    question:
      '求具有下述性质的最小正整数 t：将 100×100 的方格纸的每个小方格染为某一种颜色，若每一种颜色的小方格数目均不超过 104，则存在一个 1×t 或 t×1 的矩形，其中 t 个小方格含有至少三种不同颜色。',
    answer: '12',
  },
  {
    id: 19,
    question:
      '设实数列 {xₙ} 满足：x₀ = 0，x₂ = ∛2 · x₁，x₃ 是正整数，且 x_{n+1} = (1/∛4)·x_n + ∛4·x_{n-1} + (1/2)·x_{n-2}（n ≥ 2）。问：这类数列中最少有多少个整数项？',
    answer: '5',
  },
  {
    id: 20,
    question:
      '7 个二元随机变量初始全为 0。每步随机选一个变量翻转。若 X₁ = X₂ = 1 则成功停止；若 X₃ = X₄ = X₅ = 1 或 X₆ = X₇ = 1 则失败停止。求成功概率。',
    answer: '189213/468097',
  },
  {
    id: 21,
    question:
      '在一个黑色的袋子里放有三种口味的糖果，每种糖果有两种不同的形状（圆形和五角星形，不同的形状靠手感可以分辨）。数量如下：苹果味圆形7、桃子味圆形9、西瓜味圆形8；苹果味五角星7、桃子味五角星6、西瓜味五角星4。参赛者需在活动前决定摸出的糖果数目。问：最少取出多少个糖果，才能保证手中同时拥有不同形状的苹果味和桃子味的糖？',
    answer: '21个（策略：摸9个圆形 + 12个五角星）',
  },
]

export interface BenchmarkDialogProps {
  /** 弹窗打开状态 */
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 点击「发送」回调：参数为题目文本（不含答案） */
  onSend: (question: string) => void
  /** 是否处于发送中（禁用全部发送按钮） */
  disabled?: boolean
}

/**
 * 检测题目弹窗：表格展示 + 单题发送
 */
export function BenchmarkDialog({ open, onOpenChange, onSend, disabled }: BenchmarkDialogProps) {
  const { t } = useTranslation('chat')

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[760px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-sm">
            <i className="fa-solid fa-clipboard-question text-primary" />
            {t('benchmark.title')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {t('benchmark.description')}
          </DialogDescription>
        </DialogHeader>

        {/* 表格：内部原生滚动（弹窗内不使用 ScrollPage） */}
        <div className="max-h-[52vh] overflow-y-auto rounded-md border">
          <table className="w-full table-fixed border-collapse text-xs">
            <thead className="sticky top-0 z-10 bg-muted">
              <tr className="text-left text-muted-foreground">
                <th className="w-10 px-2 py-2 text-center font-normal">{t('benchmark.colIndex')}</th>
                <th className="px-2 py-2 font-normal">{t('benchmark.colQuestion')}</th>
                <th className="w-[180px] px-2 py-2 font-normal">{t('benchmark.colAnswer')}</th>
                <th className="w-16 px-2 py-2 text-center font-normal">{t('benchmark.colAction')}</th>
              </tr>
            </thead>
            <tbody>
              {BENCHMARK_QUESTIONS.map((q) => (
                <tr key={q.id} className="border-t align-top">
                  <td className="px-2 py-2 text-center tabular-nums text-muted-foreground">{q.id}</td>
                  <td className="break-words px-2 py-2 leading-5">{q.question}</td>
                  <td className="break-words px-2 py-2 leading-5 text-muted-foreground">{q.answer}</td>
                  <td className="px-2 py-2 text-center">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-6 px-1.5 text-xs"
                      disabled={disabled}
                      onClick={() => {
                        onOpenChange(false)
                        onSend(q.question)
                      }}
                      title={t('benchmark.send')}
                    >
                      <i className="fa-solid fa-paper-plane" />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </DialogContent>
    </Dialog>
  )
}
