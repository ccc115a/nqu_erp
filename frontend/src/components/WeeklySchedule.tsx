// 週課表元件：用 CSS Grid 畫出週一至週五 × 14 節的課表網格。
import type { CSSProperties, ReactElement } from 'react'
import type { ScheduleItem } from '../types'

// 課程區塊的樣式（藍色卡片）
const COLOR = 'bg-blue-100 border-blue-500 text-blue-900'
const DAY_NAMES = ['週一', '週二', '週三', '週四', '週五']

export default function WeeklySchedule({ items }: { items: ScheduleItem[] }) {
  // 以「星期-起始節次」為鍵建索引，讓排課塊可直接對位
  const blocks = new Map<string, ScheduleItem>()
  for (const item of items) blocks.set(`${item.day_of_week}-${item.start_period}`, item)

  const cells: ReactElement[] = []
  // 左上角空格
  cells.push(
    <div key="corner" className="flex items-center justify-center border border-gray-300 bg-gray-100 text-xs text-gray-600">
      節
    </div>,
  )
  // 星期標題列
  for (let d = 1; d <= 5; d++) {
    cells.push(
      <div key={`head-${d}`} className="flex items-center justify-center border border-gray-300 bg-gray-100 py-1 text-sm font-semibold">
        {DAY_NAMES[d - 1]}
      </div>,
    )
  }

  // 逐節列排出課表（1～14 節）
  for (let p = 1; p <= 14; p++) {
    const labelStyle: CSSProperties = {
      gridColumn: 1,
      gridRow: p + 1,
    }
    cells.push(
      <div key={`period-${p}`} style={labelStyle} className="flex items-start justify-center border border-gray-300 bg-gray-100 pt-1 text-xs text-gray-600">
        {p}
      </div>,
    )
    for (let d = 1; d <= 5; d++) {
      const item = blocks.get(`${d}-${p}`)
      if (item) {
        // 有課：跨從 start_period 到 end_period 的列數（各節用範圍合併）
        cells.push(
          <div
            key={`block-${d}-${p}`}
            style={{ gridColumn: d + 1, gridRow: `${p + 1} / ${item.end_period + 2}` }}
            className={`m-0.5 overflow-hidden rounded border p-1 text-xs ${COLOR}`}
          >
            <div className="font-semibold">{item.course_name}</div>
            <div>{item.location}</div>
            <div>{item.teacher_name}</div>
          </div>,
        )
      } else {
        // 沒課：空網格
        cells.push(
          <div key={`cell-${d}-${p}`} style={{ gridColumn: d + 1, gridRow: p + 1 }} className="min-h-[2.5rem] border border-gray-200" />,
        )
      }
    }
  }

  return (
    <div className="overflow-x-auto">
      <div
        className="grid min-w-[700px]"
        style={{ gridTemplateColumns: '40px repeat(5, 1fr)', gridAutoRows: '2.5rem' }}
      >
        {cells}
      </div>
    </div>
  )
}