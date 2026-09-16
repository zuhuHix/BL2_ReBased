#include "OpenWillowCollision.h"
#include "Engine/StaticMesh.h"
#include "PhysicsEngine/BodySetup.h"

TArray<FOpenWillowHull> UOpenWillowCollision::GetHulls(UStaticMesh* Mesh)
{
    TArray<FOpenWillowHull> Result;
    if (Mesh && Mesh->GetBodySetup())
        for (const auto& Elem : Mesh->GetBodySetup()->AggGeom.ConvexElems)
        {
            FOpenWillowHull Hull;
            Hull.Vertices = Elem.VertexData;
            Result.Add(MoveTemp(Hull));
        }
    return Result;
}

bool UOpenWillowCollision::SetHulls(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls)
{
    if (!Mesh || Hulls.Num() > 4096) return false;
    for (const auto& Hull : Hulls)
    {
        if (Hull.Vertices.Num() < 4 || Hull.Vertices.Num() > 4096) return false;
        for (const FVector& Vertex : Hull.Vertices)
            if (Vertex.ContainsNaN()) return false;
    }
    Mesh->CreateBodySetup();
    UBodySetup* Body = Mesh->GetBodySetup();
    Body->Modify();
    Body->RemoveSimpleCollision();
    Body->CollisionTraceFlag = CTF_UseSimpleAsComplex;
    for (const auto& Hull : Hulls)
    {
        FKConvexElem Elem;
        Elem.VertexData = Hull.Vertices;
        Elem.UpdateElemBox();
        Body->AggGeom.ConvexElems.Add(MoveTemp(Elem));
    }
    Body->InvalidatePhysicsData();
    Body->CreatePhysicsMeshes();
    for (const auto& Elem : Body->AggGeom.ConvexElems)
        if (!Elem.GetChaosConvexMesh()) return false;
    Mesh->MarkPackageDirty();
    return true;
}

bool UOpenWillowCollision::SetTriangleCollision(UStaticMesh* Mesh)
{
    if (!Mesh || Mesh->GetNumSourceModels() < 1 || Mesh->GetNumTriangles(0) < 1) return false;
    Mesh->CreateBodySetup();
    UBodySetup* Body = Mesh->GetBodySetup();
    Body->Modify();
    Body->RemoveSimpleCollision();
    Body->CollisionTraceFlag = CTF_UseComplexAsSimple;
    Body->InvalidatePhysicsData();
    Body->CreatePhysicsMeshes();
    Mesh->MarkPackageDirty();
    return Body->AggGeom.GetElementCount() == 0 && Body->CollisionTraceFlag == CTF_UseComplexAsSimple;
}

bool UOpenWillowCollision::HasTriangleCollision(UStaticMesh* Mesh)
{
    UBodySetup* Body = Mesh ? Mesh->GetBodySetup() : nullptr;
    return Body && Body->CollisionTraceFlag == CTF_UseComplexAsSimple && Body->AggGeom.GetElementCount() == 0;
}
