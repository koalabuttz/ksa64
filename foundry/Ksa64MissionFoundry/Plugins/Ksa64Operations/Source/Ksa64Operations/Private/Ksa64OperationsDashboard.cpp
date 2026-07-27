#include "Ksa64OperationsDashboard.h"

#include "Ksa64LiveMissionSubsystem.h"

#include "Rendering/DrawElements.h"
#include "InputCoreTypes.h"
#include "Styling/CoreStyle.h"
#include "Widgets/Images/SImage.h"
#include "Widgets/Input/SButton.h"
#include "Widgets/Layout/SBorder.h"
#include "Widgets/Layout/SBox.h"
#include "Widgets/Layout/SGridPanel.h"
#include "Widgets/Layout/SSpacer.h"
#include "Widgets/Layout/SSplitter.h"
#include "Widgets/Layout/SUniformGridPanel.h"
#include "Widgets/SBoxPanel.h"
#include "Widgets/Text/STextBlock.h"

namespace
{
const FLinearColor Background(0.012f, 0.020f, 0.035f, 1.0f);
const FLinearColor PanelDark(0.025f, 0.043f, 0.065f, 0.98f);
const FLinearColor PanelHighContrast(0.005f, 0.008f, 0.012f, 1.0f);
const FLinearColor Cyan(0.14f, 0.83f, 0.95f, 1.0f);
const FLinearColor Amber(1.0f, 0.66f, 0.18f, 1.0f);
const FLinearColor Green(0.31f, 0.93f, 0.57f, 1.0f);
const FLinearColor Red(1.0f, 0.27f, 0.25f, 1.0f);
const FLinearColor Muted(0.47f, 0.58f, 0.68f, 1.0f);
const FLinearColor White(0.91f, 0.95f, 0.98f, 1.0f);
const FMargin PanelPadding(14.0f, 11.0f);
}

void SKsa64OperationsPlot::Construct(const FArguments& Args)
{
    Subsystem = Args._Subsystem;
    PlotKind = Args._PlotKind;
}

FVector2D SKsa64OperationsPlot::ComputeDesiredSize(float LayoutScaleMultiplier) const
{
    return FVector2D(620.0f, 245.0f);
}

int32 SKsa64OperationsPlot::OnPaint(
    const FPaintArgs& Args,
    const FGeometry& AllottedGeometry,
    const FSlateRect& MyCullingRect,
    FSlateWindowElementList& OutDrawElements,
    int32 LayerId,
    const FWidgetStyle& InWidgetStyle,
    bool bParentEnabled) const
{
    const FSlateBrush* WhiteBrush = FCoreStyle::Get().GetBrush(TEXT("WhiteBrush"));
    FSlateDrawElement::MakeBox(
        OutDrawElements, LayerId, AllottedGeometry.ToPaintGeometry(), WhiteBrush,
        ESlateDrawEffect::None, FLinearColor(0.008f, 0.017f, 0.027f, 1.0f));

    const FVector2D Size = AllottedGeometry.GetLocalSize();
    for (int32 Line = 1; Line < 8; ++Line)
    {
        const float X = Size.X * static_cast<float>(Line) / 8.0f;
        TArray<FVector2D> Points{FVector2D(X, 0.0f), FVector2D(X, Size.Y)};
        FSlateDrawElement::MakeLines(
            OutDrawElements, LayerId + 1, AllottedGeometry.ToPaintGeometry(), Points,
            ESlateDrawEffect::None, FLinearColor(0.08f, 0.15f, 0.20f, 0.75f), true, 1.0f);
    }
    for (int32 Line = 1; Line < 4; ++Line)
    {
        const float Y = Size.Y * static_cast<float>(Line) / 4.0f;
        TArray<FVector2D> Points{FVector2D(0.0f, Y), FVector2D(Size.X, Y)};
        FSlateDrawElement::MakeLines(
            OutDrawElements, LayerId + 1, AllottedGeometry.ToPaintGeometry(), Points,
            ESlateDrawEffect::None, FLinearColor(0.08f, 0.15f, 0.20f, 0.75f), true, 1.0f);
    }
    if (!Subsystem.IsValid())
    {
        return LayerId + 1;
    }

    struct FLogicalPoint
    {
        double X = 0.0;
        double Y = 0.0;
    };
    TArray<FLogicalPoint> Planned;
    TArray<FLogicalPoint> Onboard;
    TArray<FLogicalPoint> Ground;
    TArray<FLogicalPoint> Observed;

    const auto AppendPrediction = [this](
        const TArray<FKsa64OperationsPredictionPoint>& Source,
        TArray<FLogicalPoint>& Destination)
    {
        for (const FKsa64OperationsPredictionPoint& Point : Source)
        {
            FLogicalPoint Logical;
            if (PlotKind == EKsa64OperationsPlotKind::Altitude)
            {
                Logical.X = static_cast<double>(Point.ReleaseEpoch);
                Logical.Y = static_cast<double>(Point.AltitudeQ12Km);
            }
            else
            {
                Logical.X = static_cast<double>(Point.DownrangeQ12Km);
                Logical.Y = static_cast<double>(Point.CrossrangeQ12Km);
            }
            Destination.Add(Logical);
        }
    };
    AppendPrediction(Subsystem->GetPlannedReferencePath(), Planned);
    AppendPrediction(Subsystem->GetOnboardPredictionPath(), Onboard);
    AppendPrediction(Subsystem->GetGroundPredictionPath(), Ground);

    for (const FKsa64OperationsReleasePoint& Point : Subsystem->GetReleaseHistory())
    {
        if (!Point.bHasMissionTime)
        {
            continue;
        }
        FLogicalPoint Logical;
        if (PlotKind == EKsa64OperationsPlotKind::Altitude)
        {
            Logical.X = static_cast<double>(Point.ReleaseEpoch);
            Logical.Y = static_cast<double>(Point.AltitudeQ12Km);
        }
        else
        {
            Logical.X = static_cast<double>(Point.DownrangeQ12Km);
            Logical.Y = static_cast<double>(Point.CrossrangeQ12Km);
        }
        Observed.Add(Logical);
    }

    double MinX = TNumericLimits<double>::Max();
    double MaxX = TNumericLimits<double>::Lowest();
    double MinY = TNumericLimits<double>::Max();
    double MaxY = TNumericLimits<double>::Lowest();
    const auto AccumulateBounds = [&MinX, &MaxX, &MinY, &MaxY](const TArray<FLogicalPoint>& Series)
    {
        for (const FLogicalPoint& Point : Series)
        {
            MinX = FMath::Min(MinX, Point.X);
            MaxX = FMath::Max(MaxX, Point.X);
            MinY = FMath::Min(MinY, Point.Y);
            MaxY = FMath::Max(MaxY, Point.Y);
        }
    };
    AccumulateBounds(Planned);
    AccumulateBounds(Onboard);
    AccumulateBounds(Ground);
    AccumulateBounds(Observed);
    if (MinX == TNumericLimits<double>::Max())
    {
        return LayerId + 1;
    }
    if (MaxX <= MinX)
    {
        MaxX = MinX + 1.0;
    }
    if (MaxY <= MinY)
    {
        MaxY = MinY + 1.0;
    }

    const auto ToScreen = [MinX, MaxX, MinY, MaxY, Size](const FLogicalPoint& Point)
    {
        return FVector2D(
            static_cast<float>((Point.X - MinX) / (MaxX - MinX)) * Size.X,
            Size.Y - static_cast<float>((Point.Y - MinY) / (MaxY - MinY)) * Size.Y);
    };
    const auto DrawSeries = [&](
        const TArray<FLogicalPoint>& Series,
        const FLinearColor& Color,
        float Thickness,
        int32 SeriesLayer)
    {
        if (Series.Num() < 2)
        {
            return;
        }
        TArray<FVector2D> ScreenPoints;
        ScreenPoints.Reserve(Series.Num());
        for (const FLogicalPoint& Point : Series)
        {
            ScreenPoints.Add(ToScreen(Point));
        }
        FSlateDrawElement::MakeLines(
            OutDrawElements, SeriesLayer, AllottedGeometry.ToPaintGeometry(), ScreenPoints,
            ESlateDrawEffect::None, Color, true, Thickness);
    };
    DrawSeries(Planned, White, 1.2f, LayerId + 2);
    DrawSeries(Onboard, Cyan, 1.5f, LayerId + 3);
    DrawSeries(Ground, Amber, 1.5f, LayerId + 4);
    DrawSeries(Observed, Green, 2.3f, LayerId + 5);

    FKsa64OperationsReleasePoint VisualPoint;
    if (Subsystem->GetVisualObservedPoint(VisualPoint))
    {
        FLogicalPoint Logical;
        if (PlotKind == EKsa64OperationsPlotKind::Altitude)
        {
            Logical.X = VisualPoint.PresentationReleaseEpoch >= 0.0
                ? VisualPoint.PresentationReleaseEpoch
                : static_cast<double>(VisualPoint.ReleaseEpoch);
            Logical.Y = static_cast<double>(VisualPoint.AltitudeQ12Km);
        }
        else
        {
            Logical.X = static_cast<double>(VisualPoint.DownrangeQ12Km);
            Logical.Y = static_cast<double>(VisualPoint.CrossrangeQ12Km);
        }
        const FVector2D Marker = ToScreen(Logical);
        TArray<FVector2D> CrossA{Marker + FVector2D(-4.0f, 0.0f), Marker + FVector2D(4.0f, 0.0f)};
        TArray<FVector2D> CrossB{Marker + FVector2D(0.0f, -4.0f), Marker + FVector2D(0.0f, 4.0f)};
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 6, AllottedGeometry.ToPaintGeometry(), CrossA, ESlateDrawEffect::None, Green, true, 2.0f);
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 6, AllottedGeometry.ToPaintGeometry(), CrossB, ESlateDrawEffect::None, Green, true, 2.0f);
    }
    return LayerId + 6;
}

void SKsa64OperationsDashboard::Construct(const FArguments& Args)
{
    Subsystem = Args._Subsystem;

    ChildSlot
    [
        SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(Background)
        .Padding(FMargin(18.0f))
        [
            SNew(SVerticalBox)
            + SVerticalBox::Slot()
            .AutoHeight()
            [
                BuildHeader()
            ]
            + SVerticalBox::Slot()
            .AutoHeight()
            .Padding(0.0f, 10.0f, 0.0f, 12.0f)
            [
                BuildTransportControls()
            ]
            + SVerticalBox::Slot()
            .FillHeight(1.0f)
            [
                SNew(SSplitter)
                .PhysicalSplitterHandleSize(6.0f)
                + SSplitter::Slot()
                .Value(0.61f)
                [
                    SNew(SVerticalBox)
                    + SVerticalBox::Slot()
                    .FillHeight(0.58f)
                    [
                        BuildTrajectoryPanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.42f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        SNew(SSplitter)
                        + SSplitter::Slot()
                        .Value(0.54f)
                        [
                            BuildNavigationPanel()
                        ]
                        + SSplitter::Slot()
                        .Value(0.46f)
                        [
                            BuildTimelinePanel()
                        ]
                    ]
                ]
                + SSplitter::Slot()
                .Value(0.39f)
                [
                    SNew(SVerticalBox)
                    + SVerticalBox::Slot()
                    .FillHeight(0.27f)
                    [
                        BuildProcedurePanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.28f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        BuildUplinkPanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.22f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        BuildDispositionPanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.23f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        BuildEngineeringPanel()
                    ]
                ]
            ]
        ]
    ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildHeader()
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(FLinearColor(0.025f, 0.075f, 0.105f, 1.0f))
        .Padding(FMargin(16.0f, 11.0f))
        [
            SNew(SHorizontalBox)
            + SHorizontalBox::Slot()
            .FillWidth(1.0f)
            .VAlign(VAlign_Center)
            [
                SNew(SVerticalBox)
                + SVerticalBox::Slot()
                .AutoHeight()
                [
                    Label(
                        FText::FromString(TEXT("KSA64  /  MISSION FOUNDRY")),
                        19,
                        White)
                ]
                + SVerticalBox::Slot()
                .AutoHeight()
                [
                    Label(
                        TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::HeaderMissionText),
                        11,
                        Cyan)
                ]
            ]
            + SHorizontalBox::Slot()
            .AutoWidth()
            .HAlign(HAlign_Right)
            .VAlign(VAlign_Center)
            [
                Label(
                    TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::HeaderStateText),
                    12,
                    Green)
            ]
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildTransportControls()
{
    return SNew(SHorizontalBox)
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("BEGIN GUIDED OPS")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnStart),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanStart),
                Green)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::PauseResumeText),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnPauseResume),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Amber)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("STEP +1")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnStep),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Cyan)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("4×")),
                FOnClicked::CreateSP(
                    this,
                    &SKsa64OperationsDashboard::OnSetPace,
                    EKsa64OperationsPace::FourX),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("16×")),
                FOnClicked::CreateSP(
                    this,
                    &SKsa64OperationsDashboard::OnSetPace,
                    EKsa64OperationsPace::SixteenX),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("MAX")),
                FOnClicked::CreateSP(
                    this,
                    &SKsa64OperationsDashboard::OnSetPace,
                    EKsa64OperationsPace::Fastest),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Muted)
        ]
        + SHorizontalBox::Slot().FillWidth(1.0f)
        [
            SNew(SSpacer)
        ]
        + SHorizontalBox::Slot().AutoWidth()
        [
            BuildAccessibilityControls()
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildTrajectoryPanel()
{
    const TSharedRef<SVerticalBox> Body = SNew(SVerticalBox)
        + SVerticalBox::Slot().AutoHeight()
        [
            SNew(SHorizontalBox)
            + SHorizontalBox::Slot().FillWidth(1.0f).VAlign(VAlign_Center)
            [
                SNew(SHorizontalBox)
                + SHorizontalBox::Slot().FillWidth(0.22f).Padding(0.0f, 0.0f, 8.0f, 0.0f)
                [
                    Label(FText::FromString(TEXT("● PLANNED REFERENCE")), 8, White)
                ]
                + SHorizontalBox::Slot().FillWidth(0.25f).Padding(0.0f, 0.0f, 8.0f, 0.0f)
                [
                    Label(FText::FromString(TEXT("● ONBOARD EST PROJECTION")), 8, Cyan)
                ]
                + SHorizontalBox::Slot().FillWidth(0.25f).Padding(0.0f, 0.0f, 8.0f, 0.0f)
                [
                    Label(FText::FromString(TEXT("● GROUND EST PROJECTION")), 8, Amber)
                ]
                + SHorizontalBox::Slot().FillWidth(0.28f)
                [
                    Label(FText::FromString(TEXT("● TRACKING-DERIVED OBSERVED")), 8, Green)
                ]
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(8.0f, 0.0f, 0.0f, 0.0f)
            [
                CommandButton(
                    TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::DisplayModeText),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnDisplayMode),
                    true,
                    Cyan)
            ]
        ]
        + SVerticalBox::Slot().FillHeight(1.0f).Padding(0.0f, 9.0f, 0.0f, 0.0f)
        [
            SNew(SSplitter)
            .PhysicalSplitterHandleSize(5.0f)
            + SSplitter::Slot().Value(0.52f)
            [
                SNew(SVerticalBox)
                + SVerticalBox::Slot().AutoHeight()
                [
                    Label(FText::FromString(TEXT("ALTITUDE / MISSION TIME (32 HZ RELEASE)")), 9, Cyan)
                ]
                + SVerticalBox::Slot().FillHeight(1.0f).Padding(0.0f, 4.0f, 3.0f, 0.0f)
                [
                    SNew(SKsa64OperationsPlot)
                    .Subsystem(Subsystem)
                    .PlotKind(EKsa64OperationsPlotKind::Altitude)
                ]
            ]
            + SSplitter::Slot().Value(0.48f)
            [
                SNew(SVerticalBox)
                + SVerticalBox::Slot().AutoHeight()
                [
                    Label(FText::FromString(TEXT("GROUND TRACK / DOWNRANGE × CROSSRANGE")), 9, Amber)
                ]
                + SVerticalBox::Slot().FillHeight(1.0f).Padding(3.0f, 4.0f, 0.0f, 0.0f)
                [
                    SNew(SKsa64OperationsPlot)
                    .Subsystem(Subsystem)
                    .PlotKind(EKsa64OperationsPlotKind::GroundTrack)
                ]
            ]
        ];
    return Panel(FText::FromString(TEXT("TRAJECTORY  /  ESTIMATE-AWARE OPERATIONS")), Body, Cyan);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildNavigationPanel()
{
    return Panel(
        FText::FromString(TEXT("NAVIGATION")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::NavigationText),
            11,
            PrimaryText()),
        Cyan);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildTimelinePanel()
{
    return Panel(
        FText::FromString(TEXT("OPERATIONS TIMELINE")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::TimelineText),
            10,
            PrimaryText()),
        Amber);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildProcedurePanel()
{
    return Panel(
        FText::FromString(TEXT("ACTIVE PROCEDURE")),
        SNew(SVerticalBox)
        + SVerticalBox::Slot().AutoHeight()
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::ProcedureText),
                14,
                Amber)
        ]
        + SVerticalBox::Slot().AutoHeight().Padding(0.0f, 8.0f, 0.0f, 0.0f)
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::ProcedureGuardText),
                10,
                PrimaryText())
        ],
        Amber);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildUplinkPanel()
{
    return Panel(
        FText::FromString(TEXT("UPLINK  /  LOAD–VALIDATE–COMMIT")),
        SNew(SVerticalBox)
        + SVerticalBox::Slot().FillHeight(1.0f)
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::UplinkText),
                10,
                PrimaryText())
        ]
        + SVerticalBox::Slot().AutoHeight().Padding(0.0f, 9.0f, 0.0f, 0.0f)
        [
            SNew(SUniformGridPanel).SlotPadding(FMargin(3.0f))
            + SUniformGridPanel::Slot(0, 0)
            [
                CommandButton(
                    FText::FromString(TEXT("1  REVIEW")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnReview),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanReviewAction),
                    Cyan)
            ]
            + SUniformGridPanel::Slot(1, 0)
            [
                CommandButton(
                    FText::FromString(TEXT("2  STAGE")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnStage),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanStageAction),
                    Amber)
            ]
            + SUniformGridPanel::Slot(0, 1)
            [
                CommandButton(
                    FText::FromString(TEXT("3  COMMIT")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnCommit),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanCommitAction),
                    Green)
            ]
            + SUniformGridPanel::Slot(1, 1)
            [
                CommandButton(
                    FText::FromString(TEXT("CANCEL")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnCancel),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanCancelAction),
                    Red)
            ]
        ],
        Green);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildDispositionPanel()
{
    return Panel(
        FText::FromString(TEXT("MISSION DISPOSITION")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::DispositionText),
            10,
            Green),
        Green);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildEngineeringPanel()
{
    return Panel(
        FText::FromString(TEXT("ENGINEERING  /  INTEGRITY")),
        SNew(SVerticalBox)
        + SVerticalBox::Slot().AutoHeight()
        [
            CommandButton(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::EngineeringToggleText),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnEngineeringToggle),
                true,
                Muted)
        ]
        + SVerticalBox::Slot().FillHeight(1.0f).Padding(0.0f, 7.0f, 0.0f, 0.0f)
        [
            SNew(SBox)
            .Visibility_Lambda([this]()
            {
                return bEngineeringExpanded ? EVisibility::Visible : EVisibility::Collapsed;
            })
            [
                Label(
                    TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::EngineeringText),
                    9,
                    Muted)
            ]
        ],
        Muted);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildAccessibilityControls()
{
    return SNew(SHorizontalBox)
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("TEXT")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnTextScale),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("CONTRAST")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnHighContrast),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("MOTION")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnReducedMotion),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("SOUND")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnSoundCues),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(9.0f, 0.0f, 0.0f, 0.0f).VAlign(VAlign_Center)
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::AccessibilityText),
                9,
                Muted)
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::Panel(
    const FText& Title,
    const TSharedRef<SWidget>& Content,
    const FLinearColor& Accent) const
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor_Lambda([this]() { return PanelBackground(); })
        .Padding(PanelPadding)
        [
            SNew(SVerticalBox)
            + SVerticalBox::Slot().AutoHeight()
            [
                SNew(SHorizontalBox)
                + SHorizontalBox::Slot().AutoWidth()
                [
                    SNew(SBox)
                    .WidthOverride(4.0f)
                    .HeightOverride(18.0f)
                    [
                        SNew(SBorder)
                        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
                        .BorderBackgroundColor(Accent)
                    ]
                ]
                + SHorizontalBox::Slot().FillWidth(1.0f).Padding(9.0f, 0.0f)
                [
                    Label(Title, 11, Accent)
                ]
            ]
            + SVerticalBox::Slot().FillHeight(1.0f).Padding(0.0f, 10.0f, 0.0f, 0.0f)
            [
                Content
            ]
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::Label(
    TAttribute<FText> Text,
    int32 BaseSize,
    const FLinearColor& Color) const
{
    return SNew(STextBlock)
        .Text(Text)
        .ColorAndOpacity(Color)
        .Font_Lambda([this, BaseSize]()
        {
            return FCoreStyle::GetDefaultFontStyle(
                TEXT("Regular"),
                FMath::RoundToInt(BaseSize * TextScale()));
        })
        .AutoWrapText(true);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::CommandButton(
    TAttribute<FText> Text,
    const FOnClicked& OnClicked,
    TAttribute<bool> Enabled,
    const FLinearColor& Accent) const
{
    const TSharedRef<SButton> Button = SNew(SButton)
        .ButtonColorAndOpacity(FLinearColor(Accent.R * 0.20f, Accent.G * 0.20f, Accent.B * 0.20f, 1.0f))
        .ForegroundColor(Accent)
        .ContentPadding(FMargin(10.0f, 6.0f))
        .IsEnabled(Enabled)
        .ToolTipText(Text)
        .OnClicked(OnClicked)
        [
            SNew(STextBlock)
            .Text(Text)
            .Font_Lambda([this]()
            {
                return FCoreStyle::GetDefaultFontStyle(
                    TEXT("Bold"),
                    FMath::RoundToInt(10.0f * TextScale()));
            })
        ];
    Button->SetAccessibleBehavior(EAccessibleBehavior::Custom, Text);
    return Button;
}

FReply SKsa64OperationsDashboard::OnKeyDown(
    const FGeometry& MyGeometry,
    const FKeyEvent& InKeyEvent)
{
    const FKey Key = InKeyEvent.GetKey();
    if (Key == EKeys::SpaceBar) return OnPauseResume();
    if (Key == EKeys::Period) return OnStep();
    if (Key == EKeys::One) return OnSetPace(EKsa64OperationsPace::Realtime);
    if (Key == EKeys::Four) return OnSetPace(EKsa64OperationsPace::FourX);
    if (Key == EKeys::Zero) return OnSetPace(EKsa64OperationsPace::Fastest);
    if (Key == EKeys::E) return OnDisplayMode();
    if (Key == EKeys::D) return OnEngineeringToggle();
    return SCompoundWidget::OnKeyDown(MyGeometry, InKeyEvent);
}

FReply SKsa64OperationsDashboard::OnStart()
{
    if (Subsystem.IsValid())
    {
        Subsystem->StartGuidedOperations();
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnPauseResume()
{
    if (Subsystem.IsValid())
    {
        if (Subsystem->GetViewModel().PresentationPace == EKsa64OperationsPace::Paused)
        {
            Subsystem->ResumeRealtime();
        }
        else
        {
            Subsystem->PausePresentation();
        }
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnStep()
{
    if (Subsystem.IsValid())
    {
        Subsystem->StepOneRelease();
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnSetPace(EKsa64OperationsPace Pace)
{
    if (Subsystem.IsValid())
    {
        Subsystem->SetPace(Pace);
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnReview()
{
    if (Subsystem.IsValid()) Subsystem->ReviewAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnStage()
{
    if (Subsystem.IsValid()) Subsystem->StageAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnCommit()
{
    if (Subsystem.IsValid()) Subsystem->CommitAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnCancel()
{
    if (Subsystem.IsValid()) Subsystem->CancelAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnReducedMotion()
{
    if (Subsystem.IsValid()) Subsystem->ToggleReducedMotion();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnHighContrast()
{
    if (Subsystem.IsValid()) Subsystem->ToggleHighContrast();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnSoundCues()
{
    if (Subsystem.IsValid()) Subsystem->ToggleSoundCues();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnTextScale()
{
    if (Subsystem.IsValid()) Subsystem->CycleTextScale();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnDisplayMode()
{
    if (Subsystem.IsValid()) Subsystem->ToggleDisplayMode();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnEngineeringToggle()
{
    bEngineeringExpanded = !bEngineeringExpanded;
    return FReply::Handled();
}

FText SKsa64OperationsDashboard::HeaderMissionText() const
{
    if (!Subsystem.IsValid())
    {
        return FText::FromString(TEXT("OPERATIONS SUBSYSTEM UNAVAILABLE"));
    }
    return FText::FromString(FString::Printf(
        TEXT("KSA-G10R  ·  GNSS LOSS  ·  %s  ·  %s"),
        *Subsystem->GetMissionElapsedLabel().ToString(),
        *Subsystem->GetReleaseLabel().ToString()));
}

FText SKsa64OperationsDashboard::HeaderStateText() const
{
    if (!Subsystem.IsValid())
    {
        return FText::FromString(TEXT("OFFLINE"));
    }
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("%s   |   %s   |   %s   |   %s"),
        *View.RoleLabel,
        *View.FrameLabel,
        *Subsystem->GetPaceLabel().ToString(),
        *View.SessionStatus));
}

FText SKsa64OperationsDashboard::NavigationText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("%s\n\nONBOARD POSITION Q12  %+d  %+d  %+d\nGROUND POSITION Q12   %+d  %+d  %+d\n\nONBOARD VELOCITY Q24  %+d  %+d  %+d\nGROUND VELOCITY Q24   %+d  %+d  %+d\n\n%s"),
        *View.NavigationLabel,
        View.NavigationPositionQ12[0], View.NavigationPositionQ12[1], View.NavigationPositionQ12[2],
        View.GroundPositionQ12[0], View.GroundPositionQ12[1], View.GroundPositionQ12[2],
        View.NavigationVelocityQ24[0], View.NavigationVelocityQ24[1], View.NavigationVelocityQ24[2],
        View.GroundVelocityQ24[0], View.GroundVelocityQ24[1], View.GroundVelocityQ24[2],
        *View.CommunicationsLabel));
}

FText SKsa64OperationsDashboard::ProcedureText() const
{
    return Subsystem.IsValid()
        ? FText::FromString(Subsystem->GetViewModel().ProcedureLabel)
        : FText::GetEmpty();
}

FText SKsa64OperationsDashboard::ProcedureGuardText() const
{
    return Subsystem.IsValid()
        ? FText::FromString(Subsystem->GetViewModel().ProcedureGuard)
        : FText::GetEmpty();
}

FText SKsa64OperationsDashboard::UplinkText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("%s\n\n%s\n\nActions are accepted only as Rust-generated typed proposals. "
             "No direct effector command and no K-format parsing exists here."),
        *View.UplinkLabel,
        *View.ActionReceiptLabel));
}

FText SKsa64OperationsDashboard::TimelineText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const TArray<FKsa64OperationsTimelineItem>& Items = Subsystem->GetTimeline();
    FString Text;
    const int32 First = FMath::Max(0, Items.Num() - 7);
    for (int32 Index = First; Index < Items.Num(); ++Index)
    {
        const FKsa64OperationsTimelineItem& Item = Items[Index];
        Text += FString::Printf(
            TEXT("%06u  %-10s  %s%s"),
            Item.ReleaseEpoch,
            *Item.Category.Left(10),
            Item.bAttention ? TEXT("◆ ") : TEXT("· "),
            *Item.Summary);
        if (Index + 1 < Items.Num()) Text += TEXT("\n");
    }
    return FText::FromString(Text.IsEmpty() ? TEXT("No operational events observed") : Text);
}

FText SKsa64OperationsDashboard::DispositionText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    if (!View.Capabilities.bDisposition)
    {
        return FText::FromString(
            TEXT("MISSION       —\nVEHICLE       —\nPROCEDURE     —\nOPERATOR      —\nAVIONICS      —\nEVIDENCE      —\n\n"
                 "Awaiting Rust-derived disposition view; procedure conformance is not mission success."));
    }
    return FText::FromString(View.DispositionLabel);
}

FText SKsa64OperationsDashboard::EngineeringText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("BRIDGE      %s\nROLE FILTER %s\nPUBLICATION %llu / %d\n"
             "QUEUES      CMD %u/%u  EVENT %u/%u  SAMPLE %u/%u\n"
             "WORKER      %u  FINALIZE %u  OVERFLOW %u\n"
             "EVIDENCE    %08X  %llu bytes  CRC %08X\n"
             "STATUS      %s\n"
             "CHECKSUMS   %08X  %08X  %08X\nOBSERVED    %s\nDIAGNOSTIC  %s"),
        *View.BridgeStatus,
        View.bTruthFiltered ? TEXT("TRUTH FILTERED") : TEXT("SIM DIRECTOR TRUTH"),
        static_cast<unsigned long long>(View.CommandSequence),
        View.CommandResult,
        View.CommandsPending, View.CommandCapacity,
        View.TimelinePending, View.TimelineCapacity,
        View.SamplesPending, View.SampleCapacity,
        View.WorkerState, View.FinalizationState, View.TransportOverflow,
        View.EvidenceIdentity, static_cast<unsigned long long>(View.EvidenceLength), View.EvidenceCrc32,
        *View.EvidenceStatus,
        View.FlightChecksum,
        View.NavigationChecksum,
        View.CommandChecksum,
        View.bObservationComplete ? TEXT("COMPLETE PREFIX") : TEXT("BOUNDED PREFIX"),
        *View.LastDiagnostic));
}

FText SKsa64OperationsDashboard::PauseResumeText() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().PresentationPace == EKsa64OperationsPace::Paused
        ? FText::FromString(TEXT("RESUME 1×"))
        : FText::FromString(TEXT("PAUSE"));
}

FText SKsa64OperationsDashboard::AccessibilityText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsAccessibilitySettings& Access = Subsystem->GetAccessibility();
    return FText::FromString(FString::Printf(
        TEXT("%.0f%%  ·  %s  ·  %s  ·  %s"),
        Access.TextScale * 100.0f,
        Access.bHighContrast ? TEXT("HIGH CONTRAST") : TEXT("STANDARD"),
        Access.bReducedMotion ? TEXT("REDUCED MOTION") : TEXT("SMOOTH"),
        Access.bSoundCues ? TEXT("CUES ON") : TEXT("CUES OFF")));
}

FText SKsa64OperationsDashboard::DisplayModeText() const
{
    if (!Subsystem.IsValid())
    {
        return FText::FromString(TEXT("VIEW EXACT"));
    }
    const bool bExact = Subsystem->GetDisplayMode() == EKsa64OperationsDisplayMode::Exact;
    return FText::FromString(bExact ? TEXT("VIEW EXACT") : TEXT("VIEW SMOOTH"));
}

FText SKsa64OperationsDashboard::EngineeringToggleText() const
{
    return FText::FromString(
        bEngineeringExpanded ? TEXT("HIDE ENGINEERING DETAILS") : TEXT("SHOW ENGINEERING DETAILS"));
}

bool SKsa64OperationsDashboard::HasSession() const
{
    return IsRunnableSession();
}

bool SKsa64OperationsDashboard::IsRunnableSession() const
{
    if (!Subsystem.IsValid())
    {
        return false;
    }
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return View.bSessionOpen
        && View.Capabilities.bTypedActions
        && !View.bShutdownRequested
        && View.Lifecycle != 5
        && View.Lifecycle != 6
        && View.WorkerState != 3
        && View.FinalizationState != 3;
}

bool SKsa64OperationsDashboard::CanStart() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().bBridgeReady
        && !Subsystem->GetViewModel().bSessionOpen;
}

bool SKsa64OperationsDashboard::CanReviewAction() const
{
    return IsRunnableSession()
        && Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Available;
}

bool SKsa64OperationsDashboard::CanStageAction() const
{
    return IsRunnableSession()
        && Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Reviewing;
}

bool SKsa64OperationsDashboard::CanCommitAction() const
{
    return IsRunnableSession()
        && Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Staged
        && Subsystem->GetViewModel().ReleaseEpoch >= Subsystem->GetViewModel().ActionEarliestCommitEpoch;
}

bool SKsa64OperationsDashboard::CanCancelAction() const
{
    return IsRunnableSession()
        && (Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Staged
            || Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Committed);
}

float SKsa64OperationsDashboard::TextScale() const
{
    return Subsystem.IsValid() ? Subsystem->GetAccessibility().TextScale : 1.0f;
}

FLinearColor SKsa64OperationsDashboard::PanelBackground() const
{
    return Subsystem.IsValid() && Subsystem->GetAccessibility().bHighContrast
        ? PanelHighContrast
        : PanelDark;
}

FLinearColor SKsa64OperationsDashboard::PrimaryText() const
{
    return Subsystem.IsValid() && Subsystem->GetAccessibility().bHighContrast
        ? White
        : FLinearColor(0.74f, 0.82f, 0.88f, 1.0f);
}

